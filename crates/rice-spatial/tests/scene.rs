use rice_spatial::{Collider, MAX_PLANES, Plane, Scene, Vec3};
fn plane(id: u32, z: f32) -> Plane {
    Plane {
        id,
        center: Vec3(0.0, 1.7, z),
        right: Vec3(1.0, 0.0, 0.0),
        up: Vec3(0.0, 1.0, 0.0),
        width: 4.0,
        height: 2.0,
    }
}
#[test]
fn picks_nearest_front_and_maps_client_uv() {
    let mut s = Scene::default();
    s.add_plane(plane(1, 0.0));
    s.add_plane(plane(2, 2.0));
    let h = s.pick(Vec3(1.0, 2.2, 6.0), Vec3(0.0, 0.0, -2.0)).unwrap();
    assert_eq!(h.id, 2);
    assert!((h.distance - 4.0).abs() < 1e-5);
    assert_eq!((h.u, h.v), (0.75, 0.25));
    assert!(s.pick(Vec3(0.0, 1.7, -3.0), Vec3(0.0, 0.0, 1.0)).is_none());
    assert!(s.pick(Vec3(3.0, 1.7, 6.0), Vec3(0.0, 0.0, -1.0)).is_none());
}
#[test]
fn rotated_plane_keeps_uv_orientation() {
    let mut s = Scene::default();
    let mut p = plane(1, 0.0);
    p.right = Vec3(0.0, 0.0, -1.0);
    s.add_plane(p);
    let h = s.pick(Vec3(6.0, 1.7, -1.0), Vec3(-1.0, 0.0, 0.0)).unwrap();
    assert_eq!((h.u, h.v), (0.75, 0.5));
}
#[test]
fn movement_does_not_depend_on_frame_rate_or_diagonal() {
    let mut a = Scene::default();
    let mut b = Scene::default();
    let mut c = Scene::default();
    for _ in 0..60 {
        a.step(1.0 / 60.0, 1.0, 0.0, false, false);
        c.step(1.0 / 60.0, 1.0, 1.0, false, false);
    }
    for _ in 0..120 {
        b.step(1.0 / 120.0, 1.0, 0.0, false, false);
    }
    assert!((a.position.2 - b.position.2).abs() < 0.001);
    let d = c.position.minus(Vec3(0.0, 1.7, 6.0));
    assert!((d.dot(d).sqrt() - (6.0 - a.position.2)).abs() < 0.001);
}
#[test]
fn continuous_sweep_stops_thin_wall_and_slides() {
    let mut s = Scene::default();
    s.add_collider(Collider {
        min: Vec3(-10.0, 0.0, 2.0),
        max: Vec3(10.0, 10.0, 2.001),
    });
    for _ in 0..120 {
        s.step(1.0 / 60.0, 1.0, 0.4, true, false);
    }
    assert!(s.position.2 >= 2.28);
    assert!(s.position.0 > 1.0);
}
#[test]
fn jump_returns_to_floor_and_long_pause_is_bounded() {
    let mut s = Scene::default();
    s.step(0.05, 0.0, 0.0, false, true);
    assert!(s.position.1 > 1.7);
    for _ in 0..240 {
        s.step(1.0 / 120.0, 0.0, 0.0, false, false);
    }
    assert_eq!(s.position.1, 1.7);
    s.step(1000.0, 1.0, 0.0, true, false);
    assert!(6.0 - s.position.2 <= 1.11);
}
#[test]
fn invalid_inputs_do_not_poison_scene_and_storage_is_bounded() {
    let mut s = Scene::default();
    assert!(!s.set_pose(Vec3(f32::NAN, 0.0, 0.0), 0.0, 0.0));
    s.look(f32::INFINITY, 1.0);
    s.step(f32::NAN, 1.0, 1.0, true, true);
    assert_eq!(s.position, Vec3(0.0, 1.7, 6.0));
    for i in 1..=MAX_PLANES {
        s.add_plane(plane(i as u32, 0.0));
    }
    assert!(!s.add_plane(plane(200, 0.0)));
    assert!(
        s.view_projection(f32::NAN, f32::NAN)
            .iter()
            .all(|v| v.is_finite())
    );
}

#[test]
fn flight_climbs_and_lands_on_roof_without_tunnelling_through_it() {
    let mut s=Scene::default();s.set_flight(true);
    s.add_collider(Collider{min:Vec3(-2.0,0.0,-2.0),max:Vec3(2.0,8.0,2.0)});
    for _ in 0..240 {s.step_vertical(1.0/120.0,0.0,0.0,true,false,1.0);}
    assert!(s.position.1>20.0);
    for _ in 0..160 {s.step_vertical(1.0/120.0,1.0,0.0,false,false,0.0);}
    assert!(s.position.2<2.0);
    for _ in 0..360 {s.step_vertical(1.0/120.0,0.0,0.0,true,false,-1.0);}
    assert!((s.position.1-9.7).abs()<0.001);
    s.set_flight(false);for _ in 0..30 {s.step(1.0/120.0,0.0,0.0,false,false);}
    assert!((s.position.1-9.7).abs()<0.001);
}
#[test]
fn projected_pointer_ray_maps_application_pixels_at_screen_edge() {
    let mut s=Scene::default();s.add_plane(plane(7,0.0));
    let x=(1.0/6.0)/(1.3_f32*0.5).tan();
    let hit=s.screen_pick(x,0.0,1.0,1.3).unwrap();
    assert_eq!(hit.id,7);assert!((hit.u-0.75).abs()<0.0001);assert!((hit.v-0.5).abs()<0.0001);
    assert!(s.screen_pick(f32::NAN,0.0,1.0,1.3).is_none());
}

#[test]
fn oriented_sprint_sweeps_do_not_tunnel_and_preserve_rotated_sliding() {
    for yaw in [0.0_f32, std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_2, -std::f32::consts::PI] {
        let (sin,cos)=yaw.sin_cos();let mut s=Scene::default();
        s.set_pose(Vec3(6.0*sin,1.7,6.0*cos),-yaw,0.0);
        assert!(s.add_oriented_collider(Vec3(2.0*sin,5.0,2.0*cos),Vec3(200.0,10.0,0.001),yaw));
        for _ in 0..30 {s.step(0.1,1.0,0.4,true,false);}
        let local_x=cos*s.position.0-sin*s.position.2;
        let local_z=sin*s.position.0+cos*s.position.2;
        assert!(local_z>=2.279, "yaw={yaw} position={:?}",s.position);
        assert!(local_x>1.0,"rotated normal must preserve sliding");
    }
}
#[test]
fn oriented_roof_and_ceiling_use_actual_rotated_footprints() {
    let yaw=std::f32::consts::FRAC_PI_4;let (sin,cos)=yaw.sin_cos();let mut s=Scene::default();
    s.set_flight(true);s.set_pose(Vec3(6.0*sin,14.0,6.0*cos),0.0,0.0);
    assert!(s.add_oriented_collider(Vec3(0.0,4.0,0.0),Vec3(3.0,8.0,20.0),yaw));
    for _ in 0..30 {s.step_vertical(0.1,0.0,0.0,true,false,-1.0);}
    assert!((s.position.1-9.7).abs()<0.001);
    s.clear_colliders();s.set_pose(Vec3(6.0*sin,1.7,6.0*cos),0.0,0.0);
    assert!(s.add_oriented_collider(Vec3(0.0,5.1,0.0),Vec3(3.0,0.2,20.0),yaw));
    for _ in 0..30 {s.step_vertical(0.1,0.0,0.0,true,false,1.0);}
    assert!((s.position.1-4.88).abs()<0.001);
}
#[test]
fn oriented_zero_yaw_matches_legacy_axis_aligned_motion_exactly() {
    let mut a=Scene::default();let mut b=Scene::default();
    a.add_collider(Collider{min:Vec3(-10.0,0.0,2.0),max:Vec3(10.0,10.0,2.001)});
    b.add_oriented_collider(Vec3(0.0,5.0,2.0005),Vec3(20.0,10.0,0.001),0.0);
    for _ in 0..120 {a.step(1.0/60.0,1.0,0.4,true,false);b.step(1.0/60.0,1.0,0.4,true,false);}
    assert!((a.position.minus(b.position).dot(a.position.minus(b.position))).sqrt()<0.000001);
}
#[test]
fn mixed_collider_admission_shares_one_cap_and_clear_releases_both_forms() {
    let mut s=Scene::default();
    for _ in 0..256 {assert!(s.add_collider(Collider{min:Vec3(0.0,0.0,0.0),max:Vec3(1.0,1.0,1.0)}));assert!(s.add_oriented_collider(Vec3(0.0,1.0,0.0),Vec3(1.0,1.0,1.0),0.5));}
    assert!(!s.add_oriented_collider(Vec3(0.0,1.0,0.0),Vec3(1.0,1.0,1.0),0.0));
    assert!(!s.add_collider(Collider{min:Vec3(0.0,0.0,0.0),max:Vec3(1.0,1.0,1.0)}));
    s.clear_colliders();assert!(s.add_oriented_collider(Vec3(0.0,1.0,0.0),Vec3(1.0,1.0,1.0),0.0));
    for (size,yaw) in [(Vec3(0.0,1.0,1.0),0.0),(Vec3(1.0,1.0,1.0),f32::NAN),(Vec3(1.0,1.0,1.0),4.0)] {assert!(!s.add_oriented_collider(Vec3(0.0,1.0,0.0),size,yaw));}
}

#[test]
fn leaving_high_flight_accelerates_without_snapping_and_reaches_floor() {
    for altitude in [1200.0_f32, 5000.0] {
        let mut s=Scene::default();s.set_flight(true);s.set_pose(Vec3(0.0,altitude,0.0),0.0,0.0);s.set_flight(false);
        assert_eq!(s.position.1,altitude,"flight switch never changes position");
        s.step(0.1,0.0,0.0,false,false);let first=altitude-s.position.1;assert!(first>0.02&&first<0.1,"first descent is physical, {first}");
        for _ in 0..9 {s.set_flight(false);s.step(0.1,0.0,0.0,false,false);}
        assert!(altitude-s.position.1>5.8,"float rounding cannot repeatedly reset gravity at {altitude}");
        for _ in 0..300 {s.step(0.1,0.0,0.0,false,false);}
        assert_eq!(s.position.1,1.7);
    }
}
#[test]
fn high_flight_fall_lands_on_real_axis_aligned_and_rotated_roofs() {
    for oriented in [false,true] {
        let mut s=Scene::default();s.set_flight(true);s.set_pose(Vec3(0.0,1200.0,0.0),0.0,0.0);
        if oriented {assert!(s.add_oriented_collider(Vec3(0.0,6.0,0.0),Vec3(4.0,12.0,8.0),0.63));}
        else {assert!(s.add_collider(Collider{min:Vec3(-2.0,0.0,-4.0),max:Vec3(2.0,12.0,4.0)}));}
        s.set_flight(false);for _ in 0..200 {s.step(0.1,0.0,0.0,false,false);}
        assert!((s.position.1-13.7).abs()<0.001,"fall sweeps onto roof, not through it");
        s.step(0.05,0.0,0.0,false,true);assert!(s.position.1>13.7,"landed walking player can jump");
    }
}
