//! Shared layouts for native and X11 windows. Geometry stays in the compositor.
use crate::scrolling_workspace::ScrollingLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis { Horizontal, Vertical }

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node<T> {
    Leaf(T),
    Split { axis: Axis, ratio: u32, first: Box<Self>, second: Box<Self> },
}

#[derive(Clone, Debug)]
pub struct TileLayout<T> {
    root: Option<Node<T>>, alternate: Option<Node<T>>, master: bool, master_ratio: u32,
    minimums: Vec<(T,u16,u16)>,
    pub scrolling: bool, pub strip: ScrollingLayout<T>,
}

impl<T> Default for TileLayout<T> {
    fn default() -> Self { Self { root: None, alternate: None, master: false, master_ratio: 6000, minimums: Vec::new(), scrolling: false, strip: ScrollingLayout::default() } }
}

impl<T: Copy + Eq> TileLayout<T> {
    /// Constraints belong to current live identities, not the saved tree.
    /// Values are outer cell extents (including the caller's gaps/chrome).
    pub fn set_minimums(&mut self, minimums: Vec<(T,u16,u16)>) {
        self.minimums=minimums;
    }
    pub fn retain_live(&mut self, live: &[T], area: Rect) {
        let scrolling=self.scrolling;self.scrolling=false;
        self.strip.retain(live);
        let original=self.master;
        for master in [false,true] {
            self.set_master(master);
            let members: Vec<_>=self.rectangles(area).into_iter().map(|(id,_)|id).filter(|id|live.contains(id)).collect();
            self.sync(&members,area);
        }
        self.set_master(original);self.scrolling=scrolling;
    }
    /// Versioned preorder snapshot. Leaf tokens belong to the caller's stable
    /// identity table, never implicitly to a compositor window-id namespace.
    pub fn snapshot(&self, mut token: impl FnMut(T) -> u32) -> Vec<u32> {
        let mut words=vec![3,u32::from(self.master),self.master_ratio];
        Node::encode_optional(self.root.as_ref(),&mut token,&mut words);
        Node::encode_optional(self.alternate.as_ref(),&mut token,&mut words);
        self.strip.encode(&mut token,&mut words);
        words
    }

    /// Decode completely before replacing live state. Both arrangements may
    /// name a window, but duplicate leaves within one arrangement are invalid.
    pub fn from_snapshot(words: &[u32], resolve: impl Fn(u32) -> Option<T>) -> Option<Self> {
        if words.len()<5 || words.len()>4096 || ![1,2,3].contains(&words[0]) || words[1]>1 || !(1..10000).contains(&words[2]) { return None; }
        let mut cursor=3;
        let root=Node::decode_optional(words,&mut cursor,&resolve,&mut Vec::new(),0)?;
        let alternate=Node::decode_optional(words,&mut cursor,&resolve,&mut Vec::new(),0)?;
        let strip=if words[0]>=2 {ScrollingLayout::decode(words,&mut cursor,&resolve,words[0])?}else{ScrollingLayout::default()};
        if cursor!=words.len() {return None;}
        // Master trees have a single left leaf and a vertical stack on the
        // right. Reject a mislabeled arbitrary split tree rather than letting
        // the next membership change silently reinterpret its shape.
        let master_root=if words[1]==1 {&root}else{&alternate};
        if master_root.as_ref().is_some_and(|node|!node.is_master()) {return None;}
        if let Some(Node::Split{ratio,..})=master_root {if *ratio!=words[2] {return None;}}
        Some(Self{root,alternate,master:words[1]==1,master_ratio:words[2],minimums:Vec::new(),scrolling:false,strip})
    }

    pub fn set_master(&mut self, master: bool) {
        if self.master != master {
            core::mem::swap(&mut self.root, &mut self.alternate);
            self.master = master;
        }
    }
    pub fn contains(&self, id: T) -> bool { if self.scrolling {return self.strip.contains(id);} self.root.as_ref().is_some_and(|n| n.contains(id)) }

    pub fn sync(&mut self, members: &[T], area: Rect) {
        if self.scrolling {self.strip.sync(members,area,&self.minimums);return;}
        if self.master {
            let previous: Vec<_> = self.rectangles(area).into_iter().map(|(id,_)|id).collect();
            let mut order: Vec<_> = previous.iter().copied().filter(|id|members.contains(id)).collect();
            for &id in members { if !order.contains(&id) { order.push(id); } }
            if previous != order {
                self.root = match order.as_slice() {
                    [] => None,
                    [id] => Some(Node::Leaf(*id)),
                    [first, rest @ ..] => Some(Node::Split {
                        axis: Axis::Horizontal, ratio: self.master_ratio,
                        first: Box::new(Node::Leaf(*first)),
                        second: Box::new(Node::stack(rest)),
                    }),
                };
            }
            return;
        }
        self.root = self.root.take().and_then(|node| node.retain(members));
        for &id in members {
            if self.contains(id) { continue; }
            if self.root.is_none() { self.root = Some(Node::Leaf(id)); continue; }
            // Split the largest tile; use the later leaf on equal area so new
            // windows enter beside the most recently created equal-size tile.
            let cells = self.rectangles(area);
            let &(target, rect) = cells.iter().max_by_key(|(_,r)| u32::from(r.width)*u32::from(r.height)).unwrap();
            let axis = if rect.width >= rect.height { Axis::Horizontal } else { Axis::Vertical };
            self.root.as_mut().unwrap().insert(target, id, axis);
        }
    }

    pub fn rectangles(&self, area: Rect) -> Vec<(T, Rect)> {
        if self.scrolling {return self.strip.rectangles(area,&self.minimums);}
        let mut output = Vec::new();
        if let Some(root) = &self.root { root.layout(area, &mut output, &self.minimums); }
        if self.overflows(area) {for (_,rect) in &mut output {*rect=area;}}
        output
    }
    /// Preserve the requested tree while presenting an explicit full-area stack
    /// when its minimum extents cannot fit. Expanding the area restores it.
    pub fn overflows(&self, area: Rect) -> bool {
        if self.scrolling {return false;}
        self.root.as_ref().is_some_and(|root|{let (width,height)=root.minimum(&self.minimums);width>u32::from(area.width)||height>u32::from(area.height)})
    }

    pub fn swap(&mut self, a: T, b: T) -> bool {
        if self.scrolling {return self.strip.swap(a,b);}
        if a == b || !self.contains(a) || !self.contains(b) { return false; }
        self.root.as_mut().unwrap().swap(a,b);
        true
    }

    /// Move a leaf beside a target, collapsing its former empty branch. Master
    /// stack keeps its topology and divider ratios while shifting slot order.
    pub fn insert_beside(&mut self, source: T, target: T, direction: u32, area: Rect) -> bool {
        if self.scrolling {return self.strip.insert(source,target,direction,area,&self.minimums);}
        if direction>3||source==target||!self.contains(source)||!self.contains(target) {return false;}
        let before=direction==0||direction==2;
        let mut root=self.root.clone().unwrap();
        if self.master {
            let mut order:Vec<_>=self.rectangles(area).into_iter().map(|(id,_)|id).filter(|id|*id!=source).collect();
            let index=order.iter().position(|id|*id==target).unwrap()+usize::from(!before);
            order.insert(index,source);root.assign_order(&order,&mut 0);
        } else {
            let members:Vec<_>=self.rectangles(area).into_iter().map(|(id,_)|id).filter(|id|*id!=source).collect();
            root=root.retain(&members).unwrap();
            root.insert_at(target,source,if direction<2 {Axis::Horizontal}else{Axis::Vertical},before);
        }
        let (width,height)=root.minimum(&self.minimums);
        if width>u32::from(area.width)||height>u32::from(area.height)||self.root.as_ref()==Some(&root) {return false;}
        self.root=Some(root);true
    }

    pub fn insertion_preview(&self, source: T, target: T, direction: u32, area: Rect) -> Option<Rect> {
        let mut next=self.clone();
        if !next.insert_beside(source,target,direction,area) {return None;}
        next.rectangles(area).into_iter().find_map(|(id,rect)|(id==source).then_some(rect))
    }

    pub fn scrolling_preset(&mut self,id:T,operation:u32,area:Rect)->bool {
        self.scrolling && self.strip.preset(id,operation,area,&self.minimums)
    }
    pub fn resize(&mut self, id: T, axis: Axis, delta: i32, area: Rect) -> bool {
        if self.scrolling {return self.strip.resize(id,axis,delta,area,&self.minimums);}
        if self.overflows(area) {return false;}
        let changed = self.root.as_mut().is_some_and(|root| root.resize(id,axis,delta,area,&self.minimums));
        if self.master {
            if let Some(Node::Split {axis: Axis::Horizontal,ratio,..}) = &self.root { self.master_ratio = *ratio; }
        }
        changed
    }

    /// Divider tokens are local to an unchanged tree. The input owner cancels
    /// the gesture if membership or arrangement changes while dragging.
    pub fn divider_at(&self, x: i32, y: i32, area: Rect) -> u32 {
        self.dividers(area).iter().enumerate().find_map(|(index,(axis,rect,position))| {
            let (along,cross,start,extent)=if *axis==Axis::Horizontal {(x,y,rect.y,rect.height)}else{(y,x,rect.x,rect.width)};
            ((i64::from(along)-i64::from(*position)).abs()<=3 && cross>=i32::from(start) && cross<i32::from(start)+i32::from(extent))
                .then_some(((index as u32+1)<<1)|u32::from(*axis==Axis::Vertical))
        }).unwrap_or(0)
    }
    fn dividers(&self, area: Rect) -> Vec<(Axis,Rect,i32)> {
        if self.scrolling {return self.strip.dividers(area,&self.minimums);}
        let mut output=Vec::new();
        if self.overflows(area) {return output;}
        if let Some(root)=&self.root {root.dividers(area,&mut output,&self.minimums);}
        output
    }
    pub fn divider_position(&self, token: u32, area: Rect) -> Option<i32> {
        let index=(token>>1).checked_sub(1)? as usize;
        let (axis,_,position)=*self.dividers(area).get(index)?;
        ((token&1)==u32::from(axis==Axis::Vertical)).then_some(position)
    }
    pub fn move_divider(&mut self, token: u32, position: i32, area: Rect) -> bool {
        if self.scrolling {return self.strip.move_divider(token,position,area,&self.minimums);}
        if self.divider_position(token,area).is_none() {return false;}
        let changed=self.root.as_mut().and_then(|root|root.move_divider((token>>1)-1,&mut 0,position,area,&self.minimums)).unwrap_or(false);
        if self.master {
            if let Some(Node::Split{ratio,..})=&self.root {self.master_ratio=*ratio;}
        }
        changed
    }
}

impl<T: Copy + Eq> Node<T> {
    fn encode_optional(node: Option<&Self>, token: &mut impl FnMut(T)->u32, words: &mut Vec<u32>) {
        match node {
            None=>words.push(0),
            Some(Self::Leaf(id))=>words.extend([1,token(*id)]),
            Some(Self::Split{axis,ratio,first,second})=>{
                words.extend([if *axis==Axis::Horizontal {2}else{3},*ratio]);
                Self::encode_optional(Some(first),token,words);
                Self::encode_optional(Some(second),token,words);
            },
        }
    }

    fn decode_optional(words: &[u32], cursor: &mut usize, resolve: &impl Fn(u32)->Option<T>, seen: &mut Vec<T>, depth: usize) -> Option<Option<Self>> {
        if depth>128 {return None;}
        let kind=*words.get(*cursor)?;*cursor+=1;
        if kind==0 {return Some(None);}
        let value=*words.get(*cursor)?;*cursor+=1;
        match kind {
            1=>{
                let id=resolve(value)?;
                if seen.len()>=128 || seen.contains(&id) {return None;}
                seen.push(id);Some(Some(Self::Leaf(id)))
            },
            2|3 if (1..10000).contains(&value)=>{
                let first=Self::decode_optional(words,cursor,resolve,seen,depth+1)??;
                let second=Self::decode_optional(words,cursor,resolve,seen,depth+1)??;
                Some(Some(Self::Split{axis:if kind==2 {Axis::Horizontal}else{Axis::Vertical},ratio:value,first:Box::new(first),second:Box::new(second)}))
            },
            _=>None,
        }
    }

    fn is_stack(&self) -> bool {
        match self {
            Self::Leaf(_)=>true,
            Self::Split{axis:Axis::Vertical,first,second,..}=>matches!(**first,Self::Leaf(_))&&second.is_stack(),
            _=>false,
        }
    }

    fn is_master(&self) -> bool {
        match self {
            Self::Leaf(_)=>true,
            Self::Split{axis:Axis::Horizontal,first,second,..}=>matches!(**first,Self::Leaf(_))&&second.is_stack(),
            _=>false,
        }
    }
    fn stack(ids: &[T]) -> Self {
        if ids.len() == 1 { return Self::Leaf(ids[0]); }
        Self::Split { axis: Axis::Vertical, ratio: 10000 / ids.len() as u32,
            first: Box::new(Self::Leaf(ids[0])), second: Box::new(Self::stack(&ids[1..])) }
    }
    fn contains(&self, id: T) -> bool {
        match self { Self::Leaf(value) => *value == id, Self::Split {first,second,..} => first.contains(id)||second.contains(id) }
    }

    fn retain(self, members: &[T]) -> Option<Self> {
        match self {
            Self::Leaf(id) => members.contains(&id).then_some(Self::Leaf(id)),
            Self::Split {axis,ratio,first,second} => match (first.retain(members),second.retain(members)) {
                (Some(a),Some(b)) => Some(Self::Split {axis,ratio,first:Box::new(a),second:Box::new(b)}),
                (a,b) => a.or(b),
            },
        }
    }

    fn insert(&mut self, target: T, id: T, axis: Axis) {
        self.insert_at(target,id,axis,false);
    }
    fn insert_at(&mut self, target: T, id: T, axis: Axis, before: bool) {
        match self {
            Self::Leaf(value) if *value == target => *self = Self::Split {axis,ratio:5000,first:Box::new(Self::Leaf(if before {id}else{target})),second:Box::new(Self::Leaf(if before {target}else{id}))},
            Self::Split {first,second,..} => { first.insert_at(target,id,axis,before); second.insert_at(target,id,axis,before); },
            _ => {},
        }
    }
    fn assign_order(&mut self, order: &[T], index: &mut usize) {
        match self {
            Self::Leaf(id)=>{*id=order[*index];*index+=1;},
            Self::Split{first,second,..}=>{first.assign_order(order,index);second.assign_order(order,index);},
        }
    }

    fn minimum(&self, minimums: &[(T,u16,u16)]) -> (u32,u32) {
        match self {
            Self::Leaf(id) => minimums.iter().find(|(candidate,_,_)|candidate==id).map_or((160,100),|(_,w,h)|(u32::from(*w).max(160),u32::from(*h).max(100))),
            Self::Split {axis,first,second,..} => {
                let (aw,ah)=first.minimum(minimums); let (bw,bh)=second.minimum(minimums);
                match axis {Axis::Horizontal=>(aw+bw,ah.max(bh)),Axis::Vertical=>(aw.max(bw),ah+bh)}
            },
        }
    }

    fn parts(axis: Axis, ratio: u32, first: &Self, second: &Self, area: Rect, minimums: &[(T,u16,u16)]) -> (Rect,Rect) {
        let horizontal=axis==Axis::Horizontal;
        let extent=u32::from(if horizontal {area.width} else {area.height});
        let a=first.minimum(minimums);let b=second.minimum(minimums);
        let (minimum_a,minimum_b)=if horizontal {(a.0,b.0)}else{(a.1,b.1)};
        let split=(extent*ratio+5000)/10000;
        let split=if minimum_a+minimum_b<=extent {split.clamp(minimum_a,extent-minimum_b)}else{split.min(extent)} as u16;
        if horizontal {
            (Rect{width:split,..area},Rect{x:area.x.saturating_add(i32::from(split)),width:area.width-split,..area})
        } else {
            (Rect{height:split,..area},Rect{y:area.y.saturating_add(i32::from(split)),height:area.height-split,..area})
        }
    }

    fn layout(&self, area: Rect, output: &mut Vec<(T,Rect)>, minimums: &[(T,u16,u16)]) {
        match self {
            Self::Leaf(id) => output.push((*id,area)),
            Self::Split {axis,ratio,first,second} => {
                let (a,b)=Self::parts(*axis,*ratio,first,second,area,minimums);
                first.layout(a,output,minimums);second.layout(b,output,minimums);
            },
        }
    }

    fn dividers(&self, area: Rect, output: &mut Vec<(Axis,Rect,i32)>, minimums: &[(T,u16,u16)]) {
        if let Self::Split{axis,ratio,first,second}=self {
            let (a,b)=Self::parts(*axis,*ratio,first,second,area,minimums);
            output.push((*axis,area,if *axis==Axis::Horizontal {b.x}else{b.y}));
            first.dividers(a,output,minimums);second.dividers(b,output,minimums);
        }
    }
    fn move_divider(&mut self, target: u32, index: &mut u32, position: i32, area: Rect, minimums: &[(T,u16,u16)]) -> Option<bool> {
        let Self::Split{axis,ratio,first,second}=self else {return None;};
        let (a,b)=Self::parts(*axis,*ratio,first,second,area,minimums);
        let current=*index;*index+=1;
        if current==target {
            let (ma,mb)=(first.minimum(minimums),second.minimum(minimums));
            let (start,extent,old,minimum_a,minimum_b)=if *axis==Axis::Horizontal {
                (area.x,area.width,a.width,ma.0,mb.0)
            } else {(area.y,area.height,a.height,ma.1,mb.1)};
            if minimum_a+minimum_b>u32::from(extent)||extent==0 {return Some(false);}
            let next=position.saturating_sub(i32::from(start)).clamp(minimum_a as i32,i32::from(extent)-minimum_b as i32);
            if next==i32::from(old) {return Some(false);}
            *ratio=((next*10000+i32::from(extent)/2)/i32::from(extent)) as u32;
            return Some(true);
        }
        first.move_divider(target,index,position,a,minimums).or_else(||second.move_divider(target,index,position,b,minimums))
    }

    fn swap(&mut self, a: T, b: T) {
        match self {
            Self::Leaf(id) => {if *id==a {*id=b;}else if *id==b {*id=a;}},
            Self::Split {first,second,..} => {first.swap(a,b);second.swap(a,b);},
        }
    }

    fn resize(&mut self, id: T, requested: Axis, delta: i32, area: Rect, minimums: &[(T,u16,u16)]) -> bool {
        let Self::Split {axis,ratio,first,second}=self else {return false;};
        let in_first=first.contains(id);
        if !in_first&&!second.contains(id) {return false;}
        let (a,b)=Self::parts(*axis,*ratio,first,second,area,minimums);
        if if in_first {first.resize(id,requested,delta,a,minimums)}else{second.resize(id,requested,delta,b,minimums)} {return true;}
        if *axis!=requested {return false;}
        let (ma,mb)= (first.minimum(minimums),second.minimum(minimums));
        let (extent,current,minimum_a,minimum_b)=match axis {
            Axis::Horizontal=>(i32::from(area.width),i32::from(a.width),ma.0,mb.0),
            Axis::Vertical=>(i32::from(area.height),i32::from(a.height),ma.1,mb.1),
        };
        if minimum_a+minimum_b>extent as u32 {return false;}
        let next=current.saturating_add(if in_first {delta}else{delta.saturating_neg()}).clamp(minimum_a as i32,extent-minimum_b as i32);
        if next==current||extent==0 {return false;}
        *ratio=((next*10000+extent/2)/extent) as u32;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const AREA: Rect=Rect{x:10,y:30,width:1000,height:700};
    #[test]
    fn live_minimums_follow_identity_and_do_not_rewrite_preferred_ratios() {
        let mut layout=TileLayout::default();layout.sync(&[1,2],AREA);
        let saved=layout.snapshot(|id|id);
        layout.set_minimums(vec![(1,700,100),(2,200,100)]);
        assert_eq!(layout.rectangles(AREA)[0].1.width,700);
        assert_eq!(layout.snapshot(|id|id),saved);
        assert!(layout.swap(1,2));
        let cells=layout.rectangles(AREA);
        assert_eq!(cells[1].0,1);assert_eq!(cells[1].1.width,700);
        assert!(layout.move_divider(2,i32::MIN,AREA));
        assert_eq!(layout.rectangles(AREA)[0].1.width,200);
        layout.set_minimums(vec![(1,900,100),(2,200,100)]);
        assert!(layout.overflows(AREA));assert!(layout.rectangles(AREA).iter().all(|(_,rect)|*rect==AREA));
        layout.set_minimums(Vec::new());assert!(!layout.overflows(AREA));
        let mut restored=TileLayout::from_snapshot(&saved,Some).unwrap();
        restored.set_minimums(vec![(1,700,100),(2,200,100)]);
        assert_eq!(restored.rectangles(AREA)[0].1.width,700);
    }
    #[test]
    fn overflow_stacks_without_losing_tree_ratios_or_membership() {
        for master in [false,true] {
            let mut layout=TileLayout::default();layout.set_master(master);layout.sync(&[1,2,3],AREA);
            assert!(layout.resize(1,Axis::Horizontal,32,AREA));
            let snapshot=layout.snapshot(|id|id);
            let original=layout.rectangles(AREA);
            let small=Rect{width:250,height:150,..AREA};
            assert!(layout.overflows(small));
            let stacked=layout.rectangles(small);
            assert_eq!(stacked.len(),3);assert!(stacked.iter().all(|(_,rect)|*rect==small));
            assert_eq!(layout.divider_at(135,80,small),0);
            assert!(!layout.resize(1,Axis::Horizontal,32,small));
            assert!(!layout.move_divider(2,140,small));
            assert_eq!(layout.snapshot(|id|id),snapshot);
            assert_eq!(layout.rectangles(AREA),original);
            layout.sync(&[1],small);
            assert!(!layout.overflows(small));assert_eq!(layout.rectangles(small),vec![(1,small)]);
        }
    }

    #[test]
    fn insertion_preview_matches_commit_without_mutating_original_tree() {
        let mut layout=TileLayout::default();layout.sync(&[1,2,3],AREA);
        let original=layout.snapshot(|id|id);
        let expected=Rect{x:10,y:380,width:500,height:350};
        assert_eq!(layout.insertion_preview(1,3,0,AREA),Some(expected));
        assert_eq!(layout.snapshot(|id|id),original);
        assert!(layout.insert_beside(1,3,0,AREA));
        let cells=layout.rectangles(AREA);
        assert_eq!(cells,vec![(2,Rect{x:10,y:30,width:1000,height:350}),(1,expected),(3,Rect{x:510,y:380,width:500,height:350})]);
        assert_eq!(cells.iter().map(|(_,r)|u32::from(r.width)*u32::from(r.height)).sum::<u32>(),700000);
        assert!(TileLayout::from_snapshot(&layout.snapshot(|id|id),Some).is_some());
        for (source,target,direction) in [(1,1,0),(99,1,0),(1,99,0),(1,2,4)] {
            let before=layout.snapshot(|id|id);
            assert!(!layout.insert_beside(source,target,direction,AREA));
            assert_eq!(layout.snapshot(|id|id),before);
        }
        let before=layout.snapshot(|id|id);
        assert!(!layout.insert_beside(1,2,0,Rect{width:200,..AREA}));
        assert_eq!(layout.snapshot(|id|id),before,"insufficient room leaves the tree intact");
    }

    #[test]
    fn master_insertion_shifts_slots_without_resetting_dividers() {
        let mut layout=TileLayout::default();layout.set_master(true);layout.sync(&[1,2,3],AREA);
        layout.move_divider(2,710,AREA);layout.move_divider(5,450,AREA);
        let before=layout.rectangles(AREA);
        assert!(layout.insert_beside(3,1,0,AREA));
        let after=layout.rectangles(AREA);
        assert_eq!(after.iter().map(|(id,_)|*id).collect::<Vec<_>>(),vec![3,1,2]);
        assert_eq!(after.iter().map(|(_,r)|*r).collect::<Vec<_>>(),before.iter().map(|(_,r)|*r).collect::<Vec<_>>());
        assert!(!layout.insert_beside(3,1,0,AREA));
        assert!(TileLayout::from_snapshot(&layout.snapshot(|id|id),Some).is_some());
        assert!(layout.insert_beside(3,2,3,AREA));
        assert_eq!(layout.rectangles(AREA).iter().map(|(id,_)|*id).collect::<Vec<_>>(),vec![1,2,3]);
    }

    #[test]
    fn pointer_dividers_hit_only_the_gap_and_resize_nested_splits() {
        let mut layout=TileLayout::default();layout.sync(&[1,2,3],AREA);
        let token=layout.divider_at(510,100,AREA);assert_eq!(token,2);
        assert_eq!(layout.divider_at(506,100,AREA),0);
        assert_eq!(layout.divider_at(514,100,AREA),0);
        assert_eq!(layout.divider_at(510,29,AREA),0);
        assert_eq!(layout.divider_at(i32::MIN,100,AREA),0);
        assert_eq!(layout.divider_at(750,380,AREA),5);
        assert!(layout.move_divider(5,450,AREA));
        let rectangles=layout.rectangles(AREA);
        assert_eq!(rectangles[1].1.height,420);
        assert_eq!(rectangles[2].1.y,450);
        assert!(layout.move_divider(token,610,AREA));
        assert_eq!(layout.rectangles(AREA)[0].1.width,600);
        assert!(!layout.move_divider(3,700,AREA),"wrong axis token");
        assert!(!layout.move_divider(99,700,AREA));
        assert!(layout.move_divider(token,i32::MAX,AREA));
        assert_eq!(layout.rectangles(AREA)[0].1.width,840);
        assert!(!layout.move_divider(token,i32::MAX,AREA));
        assert!(layout.move_divider(token,i32::MIN,AREA));
        assert_eq!(layout.rectangles(AREA)[0].1.width,160);
    }

    #[test]
    fn pointer_master_resize_updates_snapshot_ratio_and_survives_membership() {
        let mut layout=TileLayout::default();layout.set_master(true);layout.sync(&[1,2,3],AREA);
        assert!(layout.move_divider(2,710,AREA));
        let words=layout.snapshot(|id|id);
        assert!(TileLayout::from_snapshot(&words,Some).is_some());
        layout.sync(&[1,2,3,4],AREA);
        assert_eq!(layout.rectangles(AREA)[0].1.width,700);
        let tiny=Rect{width:100,height:80,..AREA};
        assert!(!layout.move_divider(2,50,tiny),"undersized trees do not resize past their minimum");
    }

    #[test]
    fn snapshot_remaps_identities_and_restores_both_arrangements() {
        let mut layout=TileLayout::default();
        layout.sync(&[1,2,3,4],AREA);
        layout.resize(1,Axis::Horizontal,32,AREA);
        layout.swap(1,4);
        let split=layout.rectangles(AREA);
        layout.set_master(true);layout.sync(&[1,2,3,4],AREA);
        layout.resize(1,Axis::Horizontal,-32,AREA);
        layout.resize(2,Axis::Vertical,32,AREA);
        layout.swap(1,3);
        let master=layout.rectangles(AREA);
        let words=layout.snapshot(|id|id+20);
        let mut restored=TileLayout::from_snapshot(&words,|token|(21..=24).contains(&token).then_some(token+80)).unwrap();
        let remap=|items:Vec<(u32,Rect)>|items.into_iter().map(|(id,rect)|(id+100,rect)).collect::<Vec<_>>();
        assert_eq!(restored.rectangles(AREA),remap(master));
        restored.set_master(false);
        assert_eq!(restored.rectangles(AREA),remap(split));
        restored.set_master(true);restored.sync(&[101,102,103,104,105],AREA);
        assert_eq!(restored.rectangles(AREA)[0].1.width,568);
        let smaller=Rect{width:800,height:500,..AREA};
        assert_eq!(restored.rectangles(smaller)[0].1.width,454);
    }

    #[test]
    fn snapshot_rejects_malformed_unresolved_and_aliased_windows() {
        let mut layout=TileLayout::default();layout.sync(&[1,2],AREA);
        let words=layout.snapshot(|id|id);
        for length in 0..words.len() {
            assert!(TileLayout::from_snapshot(&words[..length],Some).is_none());
        }
        let mut trailing=words.clone();trailing.push(0);
        assert!(TileLayout::from_snapshot(&trailing,Some).is_none());
        for index in [0,1,2,3,4] {
            let mut invalid=words.clone();invalid[index]=u32::MAX;
            assert!(TileLayout::from_snapshot(&invalid,Some).is_none());
        }
        assert!(TileLayout::<u32>::from_snapshot(&words,|_|None).is_none());
        assert!(TileLayout::from_snapshot(&words,|_|Some(7)).is_none());
        assert!(TileLayout::from_snapshot(&[1,0,6000,2,5000,0,1,2,0],Some).is_none());
        // Labeling a vertical pair as a master tree is invalid.
        assert!(TileLayout::from_snapshot(&[1,1,6000,3,6000,1,1,1,2,0],Some).is_none());
        let empty=TileLayout::<u32>::default().snapshot(|id|id);
        assert!(TileLayout::from_snapshot(&empty,Some).unwrap().rectangles(AREA).is_empty());
        layout.set_master(true);layout.sync(&(1..=128).collect::<Vec<_>>(),AREA);
        assert!(TileLayout::from_snapshot(&layout.snapshot(|id|id),Some).is_some());
    }

    #[test]
    fn master_stack_preserves_width_order_and_independent_split_tree() {
        let mut layout=TileLayout::default();
        layout.sync(&[1,2,3,4],AREA);
        layout.resize(1,Axis::Horizontal,32,AREA);
        let split=layout.rectangles(AREA);
        layout.set_master(true);layout.sync(&[1,2,3,4],AREA);
        let master=layout.rectangles(AREA);
        assert_eq!(master[0],(1,Rect{width:600,..AREA}));
        assert!(master[1..].iter().all(|(_,r)|r.x==610&&r.width==400));
        assert_eq!(master[1..].iter().map(|(_,r)|r.height as u32).sum::<u32>(),700);
        assert!(layout.resize(1,Axis::Horizontal,32,AREA));
        assert!(layout.swap(1,3));
        layout.sync(&[1,2,3,4,5],AREA);
        assert_eq!(layout.rectangles(AREA)[0].0,3);
        assert_eq!(layout.rectangles(AREA)[0].1.width,632);
        layout.sync(&[1,2,4,5],AREA);
        assert_eq!(layout.rectangles(AREA)[0].0,2);
        assert_eq!(layout.rectangles(AREA)[0].1.width,632);
        layout.set_master(false);layout.sync(&[1,2,3,4],AREA);
        assert_eq!(layout.rectangles(AREA),split);
        layout.set_master(true);layout.sync(&[2],AREA);
        assert_eq!(layout.rectangles(AREA),vec![(2,AREA)]);
        layout.sync(&[2,6],AREA);
        assert_eq!(layout.rectangles(AREA)[0].1.width,632);
    }

    #[test]
    fn resizing_preserves_coverage_and_survives_insert_remove_and_swap() {
        let mut layout=TileLayout::default();
        layout.sync(&[1,2],AREA);
        assert!(layout.resize(1,Axis::Horizontal,32,AREA));
        let resized=layout.rectangles(AREA);
        assert_eq!(resized[0].1.width,532);
        assert_eq!(resized[1].1.x,resized[0].1.x+i32::from(resized[0].1.width));
        assert_eq!(resized[1].1.x+i32::from(resized[1].1.width),1010);
        layout.sync(&[1,2,3],AREA);
        layout.sync(&[1,2],AREA);
        assert_eq!(layout.rectangles(AREA),resized);
        assert!(layout.swap(1,2));
        assert_eq!(layout.rectangles(AREA),vec![(2,resized[0].1),(1,resized[1].1)]);
        assert!(layout.resize(1,Axis::Horizontal,32,AREA));
        assert_eq!(layout.rectangles(AREA)[1].1.width,500);
    }

    #[test]
    fn resizing_clamps_and_selects_the_nearest_matching_split() {
        let mut layout=TileLayout::default();
        layout.sync(&[1,2,3],AREA);
        assert!(layout.resize(2,Axis::Vertical,32,AREA));
        let rectangles=layout.rectangles(AREA);
        assert_eq!(rectangles[1].1.height,382);
        assert_eq!(rectangles[2].1.height,318);
        for _ in 0..100 {layout.resize(2,Axis::Vertical,32,AREA);}
        assert_eq!(layout.rectangles(AREA)[2].1.height,100);
        assert!(!layout.resize(2,Axis::Vertical,32,AREA));
        assert!(!layout.resize(99,Axis::Horizontal,32,AREA));
        layout.sync(&[1],AREA);
        assert!(!layout.resize(1,Axis::Horizontal,32,AREA));
        assert_eq!(layout.rectangles(AREA),vec![(1,AREA)]);
    }
}
