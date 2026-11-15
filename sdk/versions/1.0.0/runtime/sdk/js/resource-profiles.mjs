// Generated from the actual shared document engine and licensed resources.
import {withPresentationFontResources} from './presentation-font-resources.mjs';
export const SDK_RESOURCE_PROFILES=withPresentationFontResources(Object.freeze({
  "schema": "ricewm.sdk-resources/v1",
  "sourceFingerprint": "5ff916a581766a5aaf2ba06e196c61030e5b3f8c03c2437fe353b92c1d56ef4b",
  "profiles": {
    "image": {
      "package": "deck",
      "source": "image.wasm",
      "packagePath": "image.wasm"
    },
    "typography": {
      "package": "deck",
      "source": "typography.wasm",
      "packagePath": "typography.wasm"
    },
    "presentation": {
      "package": "deck",
      "source": "engine.wasm",
      "packagePath": "presentation.wasm"
    },
    "workbook": {
      "package": "sheet",
      "source": "engine.wasm",
      "packagePath": "workbook.wasm"
    },
    "rich-document": {
      "engine": "document-engine",
      "packagePath": "document.wasm",
      "fontManifest": "document-fonts.json",
      "compute": {
        "memoryPages": 3072,
        "instances": 2
      },
      "fonts": [
        {
          "family": "Rice Office Sans",
          "id": 0,
          "path": "assets/fonts/office/LiberationSans-Regular.ttf",
          "bytes": 139512,
          "parts": [
            {
              "resource": "document-font-0-0.bin",
              "bytes": 139512
            }
          ]
        },
        {
          "family": "Rice Office Sans",
          "id": 1,
          "path": "assets/fonts/office/LiberationSans-Bold.ttf",
          "bytes": 137052,
          "parts": [
            {
              "resource": "document-font-1-0.bin",
              "bytes": 137052
            }
          ]
        },
        {
          "family": "Rice Office Sans",
          "id": 2,
          "path": "assets/fonts/office/LiberationSans-Italic.ttf",
          "bytes": 162036,
          "parts": [
            {
              "resource": "document-font-2-0.bin",
              "bytes": 162036
            }
          ]
        },
        {
          "family": "Rice Office Sans",
          "id": 3,
          "path": "assets/fonts/office/LiberationSans-BoldItalic.ttf",
          "bytes": 135124,
          "parts": [
            {
              "resource": "document-font-3-0.bin",
              "bytes": 135124
            }
          ]
        },
        {
          "family": "Rice Office Serif",
          "id": 4,
          "path": "assets/fonts/office/LiberationSerif-Regular.ttf",
          "bytes": 152408,
          "parts": [
            {
              "resource": "document-font-4-0.bin",
              "bytes": 152408
            }
          ]
        },
        {
          "family": "Rice Office Serif",
          "id": 5,
          "path": "assets/fonts/office/LiberationSerif-Bold.ttf",
          "bytes": 147132,
          "parts": [
            {
              "resource": "document-font-5-0.bin",
              "bytes": 147132
            }
          ]
        },
        {
          "family": "Rice Office Serif",
          "id": 6,
          "path": "assets/fonts/office/LiberationSerif-Italic.ttf",
          "bytes": 145028,
          "parts": [
            {
              "resource": "document-font-6-0.bin",
              "bytes": 145028
            }
          ]
        },
        {
          "family": "Rice Office Serif",
          "id": 7,
          "path": "assets/fonts/office/LiberationSerif-BoldItalic.ttf",
          "bytes": 151452,
          "parts": [
            {
              "resource": "document-font-7-0.bin",
              "bytes": 151452
            }
          ]
        },
        {
          "family": "Rice Office Mono",
          "id": 8,
          "path": "assets/fonts/office/LiberationMono-Regular.ttf",
          "bytes": 108172,
          "parts": [
            {
              "resource": "document-font-8-0.bin",
              "bytes": 108172
            }
          ]
        },
        {
          "family": "Rice Office Mono",
          "id": 9,
          "path": "assets/fonts/office/LiberationMono-Bold.ttf",
          "bytes": 105460,
          "parts": [
            {
              "resource": "document-font-9-0.bin",
              "bytes": 105460
            }
          ]
        },
        {
          "family": "Rice Office Mono",
          "id": 10,
          "path": "assets/fonts/office/LiberationMono-Italic.ttf",
          "bytes": 124012,
          "parts": [
            {
              "resource": "document-font-10-0.bin",
              "bytes": 124012
            }
          ]
        },
        {
          "family": "Rice Office Mono",
          "id": 11,
          "path": "assets/fonts/office/LiberationMono-BoldItalic.ttf",
          "bytes": 118296,
          "parts": [
            {
              "resource": "document-font-11-0.bin",
              "bytes": 118296
            }
          ]
        },
        {
          "family": "Rice Office CJK",
          "id": 12,
          "path": "assets/fonts/ricewm-cjk.ttf",
          "bytes": 17772300,
          "parts": [
            {
              "resource": "document-font-12-0.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-1.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-2.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-3.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-4.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-5.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-6.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-7.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-8.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-9.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-10.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-11.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-12.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-13.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-14.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-15.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-12-16.bin",
              "bytes": 995084
            }
          ]
        },
        {
          "family": "Rice Office Arabic",
          "id": 13,
          "path": "assets/fonts/locales/arabic.ttf",
          "bytes": 844676,
          "parts": [
            {
              "resource": "document-font-13-0.bin",
              "bytes": 844676
            }
          ]
        },
        {
          "family": "Rice Office Devanagari",
          "id": 14,
          "path": "assets/fonts/locales/devanagari.ttf",
          "bytes": 641944,
          "parts": [
            {
              "resource": "document-font-14-0.bin",
              "bytes": 641944
            }
          ]
        },
        {
          "family": "Rice Office Bengali",
          "id": 15,
          "path": "assets/fonts/locales/bengali.ttf",
          "bytes": 463668,
          "parts": [
            {
              "resource": "document-font-15-0.bin",
              "bytes": 463668
            }
          ]
        },
        {
          "family": "Rice Office Tamil",
          "id": 16,
          "path": "assets/fonts/locales/tamil.ttf",
          "bytes": 340668,
          "parts": [
            {
              "resource": "document-font-16-0.bin",
              "bytes": 340668
            }
          ]
        },
        {
          "family": "Rice Office Telugu",
          "id": 17,
          "path": "assets/fonts/locales/telugu.ttf",
          "bytes": 888808,
          "parts": [
            {
              "resource": "document-font-17-0.bin",
              "bytes": 888808
            }
          ]
        },
        {
          "family": "Rice Office Thai",
          "id": 18,
          "path": "assets/fonts/locales/thai.ttf",
          "bytes": 218652,
          "parts": [
            {
              "resource": "document-font-18-0.bin",
              "bytes": 218652
            }
          ]
        },
        {
          "family": "Rice Office Korean",
          "id": 19,
          "path": "assets/fonts/locales/kr.ttf",
          "bytes": 10414588,
          "parts": [
            {
              "resource": "document-font-19-0.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-1.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-2.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-3.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-4.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-5.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-6.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-7.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-8.bin",
              "bytes": 1048576
            },
            {
              "resource": "document-font-19-9.bin",
              "bytes": 977404
            }
          ]
        }
      ],
      "licenses": [
        "document-license-0",
        "document-license-1",
        "document-license-2",
        "document-license-3",
        "document-license-4",
        "document-license-5",
        "document-license-6",
        "document-license-7",
        "document-license-8"
      ]
    }
  },
  "files": [
    {
      "key": "document-engine",
      "path": "sdk/resources/document/document.wasm",
      "bytes": 6621688,
      "sha256": "f32ce9ea0b59e27b755ce392a4ec30ba6d03345e30d077147c226a38940136a3",
      "url": new URL("../resources/document/document.wasm",import.meta.url).href
    },
    {
      "key": "document-font-0-0.bin",
      "bytes": 139512,
      "sha256": "f8ace1f892b2bd9dc1792ba7f097fa7588f84fed48321480e04de5390828221f",
      "sharedFont": {
        "id": 0,
        "offset": 0
      }
    },
    {
      "key": "document-font-1-0.bin",
      "bytes": 137052,
      "sha256": "361c61b82d575c5c35fd9157fda8b0194bcfcd0d88ea8521a4fb5dd53d33dddc",
      "sharedFont": {
        "id": 1,
        "offset": 0
      }
    },
    {
      "key": "document-font-2-0.bin",
      "bytes": 162036,
      "sha256": "832b4406dbef23628800d3aaad21048534ac84d7e3ad955be83b8172ed8ef512",
      "sharedFont": {
        "id": 2,
        "offset": 0
      }
    },
    {
      "key": "document-font-3-0.bin",
      "bytes": 135124,
      "sha256": "a224075ac17495ad0a3af3bc0a419ac0704a8b3fd1095456201fb9b095fc281d",
      "sharedFont": {
        "id": 3,
        "offset": 0
      }
    },
    {
      "key": "document-font-4-0.bin",
      "bytes": 152408,
      "sha256": "1c9c77c2cd0f3c2d2aeef53ea50a4d5d3d684ac73a431d7c70d7864887d194a3",
      "sharedFont": {
        "id": 4,
        "offset": 0
      }
    },
    {
      "key": "document-font-5-0.bin",
      "bytes": 147132,
      "sha256": "af228b9de767e5162168e433351b3d0e086648219c6c12ed888fba0bda3694d5",
      "sharedFont": {
        "id": 5,
        "offset": 0
      }
    },
    {
      "key": "document-font-6-0.bin",
      "bytes": 145028,
      "sha256": "ccd19036f8e8599e920345105df8d21eb27a66b700ac23968fb14d70259ddb1e",
      "sharedFont": {
        "id": 6,
        "offset": 0
      }
    },
    {
      "key": "document-font-7-0.bin",
      "bytes": 151452,
      "sha256": "19c8cef87d61197323b0501d0f8676669489658f23417ca661de18d85755af06",
      "sharedFont": {
        "id": 7,
        "offset": 0
      }
    },
    {
      "key": "document-font-8-0.bin",
      "bytes": 108172,
      "sha256": "a2e642d350acb1c5e368f375db8a4ef5a2f4ff2b79056d8c3be8bd8a7e9fcb44",
      "sharedFont": {
        "id": 8,
        "offset": 0
      }
    },
    {
      "key": "document-font-9-0.bin",
      "bytes": 105460,
      "sha256": "267ae0b63fc67980d11fc4cbb630de84d845c6bf350988ec56766baf98706b8b",
      "sharedFont": {
        "id": 9,
        "offset": 0
      }
    },
    {
      "key": "document-font-10-0.bin",
      "bytes": 124012,
      "sha256": "8d39f087bf0d103586624233981c048818e1d4bcb74e4c5ca96a34f6caed583d",
      "sharedFont": {
        "id": 10,
        "offset": 0
      }
    },
    {
      "key": "document-font-11-0.bin",
      "bytes": 118296,
      "sha256": "c8f21356b3948b9ae2858b82a5514a8384c53847fde1f9ecd38cb6849582f864",
      "sharedFont": {
        "id": 11,
        "offset": 0
      }
    },
    {
      "key": "document-font-12-0.bin",
      "bytes": 1048576,
      "sha256": "38b95b4f80d00d495aa7e7bf14beede1d231e2a019e24c7d28e0c1a000829460",
      "sharedFont": {
        "id": 12,
        "offset": 0
      }
    },
    {
      "key": "document-font-12-1.bin",
      "bytes": 1048576,
      "sha256": "5c186b7291c8312a6a10de6041c1229d4db90b00b3ac964e804b15dc779263c2",
      "sharedFont": {
        "id": 12,
        "offset": 1048576
      }
    },
    {
      "key": "document-font-12-2.bin",
      "bytes": 1048576,
      "sha256": "ea95436437fa236ea7cd26751451799c1633332a73eaebb595b0a5e5fa46722e",
      "sharedFont": {
        "id": 12,
        "offset": 2097152
      }
    },
    {
      "key": "document-font-12-3.bin",
      "bytes": 1048576,
      "sha256": "ad10c0c3d8671011d7895afbcebb6de78d763f74eb2f416a86f6ee6109564fd4",
      "sharedFont": {
        "id": 12,
        "offset": 3145728
      }
    },
    {
      "key": "document-font-12-4.bin",
      "bytes": 1048576,
      "sha256": "4bb10a1cfb3003ef5cc895fef0848846eed082f4e9c935506002ea10cd419f75",
      "sharedFont": {
        "id": 12,
        "offset": 4194304
      }
    },
    {
      "key": "document-font-12-5.bin",
      "bytes": 1048576,
      "sha256": "67f08f61154d390a04158573758de40a5982ad892a55d8996c00e9b27e5babcd",
      "sharedFont": {
        "id": 12,
        "offset": 5242880
      }
    },
    {
      "key": "document-font-12-6.bin",
      "bytes": 1048576,
      "sha256": "d819d13fc5224f78e8953b1aa9a23a39a5e63f20c4f0f1f4ab84a12929efbee1",
      "sharedFont": {
        "id": 12,
        "offset": 6291456
      }
    },
    {
      "key": "document-font-12-7.bin",
      "bytes": 1048576,
      "sha256": "964fe3cfeef492bd60ef5de6875cd55bd6de1b647a4c592d263fa27bd99d3ca9",
      "sharedFont": {
        "id": 12,
        "offset": 7340032
      }
    },
    {
      "key": "document-font-12-8.bin",
      "bytes": 1048576,
      "sha256": "6c26a79e518e11abbf6df62ead677233a740a25b9f60fff85f37e4c4c380e8ce",
      "sharedFont": {
        "id": 12,
        "offset": 8388608
      }
    },
    {
      "key": "document-font-12-9.bin",
      "bytes": 1048576,
      "sha256": "3b043fac7ad33f4243b9cae26d2ccb9b922234c800a96811e074147d0e01418d",
      "sharedFont": {
        "id": 12,
        "offset": 9437184
      }
    },
    {
      "key": "document-font-12-10.bin",
      "bytes": 1048576,
      "sha256": "f9714d3406cac59d747c19c823ceed8a46893d303ff203adb29598eb3b27400f",
      "sharedFont": {
        "id": 12,
        "offset": 10485760
      }
    },
    {
      "key": "document-font-12-11.bin",
      "bytes": 1048576,
      "sha256": "c45764dc49d27977122d05ae5887d4e4f64988685d77f47b3d311d7e392800c1",
      "sharedFont": {
        "id": 12,
        "offset": 11534336
      }
    },
    {
      "key": "document-font-12-12.bin",
      "bytes": 1048576,
      "sha256": "a3fe323852ec3f70c5b6ac4c1f818f9c746f9b0eec211df83feb75e15b9c044d",
      "sharedFont": {
        "id": 12,
        "offset": 12582912
      }
    },
    {
      "key": "document-font-12-13.bin",
      "bytes": 1048576,
      "sha256": "cf174800b42ab817223165bbf0f1188b519a337aa65c69a8ec641ce38fdff980",
      "sharedFont": {
        "id": 12,
        "offset": 13631488
      }
    },
    {
      "key": "document-font-12-14.bin",
      "bytes": 1048576,
      "sha256": "d33ceb885f4ad90c4540adfe76b156f9a86327361b055087daf0661d7858cb0c",
      "sharedFont": {
        "id": 12,
        "offset": 14680064
      }
    },
    {
      "key": "document-font-12-15.bin",
      "bytes": 1048576,
      "sha256": "d5a8855ec2df7de9bbfe0d37beace76a00300dee4ead2ae460f3c2832bb32e5b",
      "sharedFont": {
        "id": 12,
        "offset": 15728640
      }
    },
    {
      "key": "document-font-12-16.bin",
      "bytes": 995084,
      "sha256": "78b207a6a931245d82369943782f13eea0da60bfbf2f029bfe9afc29915cf5bf",
      "sharedFont": {
        "id": 12,
        "offset": 16777216
      }
    },
    {
      "key": "document-font-13-0.bin",
      "bytes": 844676,
      "sha256": "63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e",
      "sharedFont": {
        "id": 13,
        "offset": 0
      }
    },
    {
      "key": "document-font-14-0.bin",
      "bytes": 641944,
      "sha256": "14ec4af41f27482216d1c2229f417ff9b1425e1babb014e57d1d40d03229853e",
      "sharedFont": {
        "id": 14,
        "offset": 0
      }
    },
    {
      "key": "document-font-15-0.bin",
      "bytes": 463668,
      "sha256": "dcd42978094e584a849c84a51450eeac40c8826057d566ea6d4b9627a403a05a",
      "sharedFont": {
        "id": 15,
        "offset": 0
      }
    },
    {
      "key": "document-font-16-0.bin",
      "bytes": 340668,
      "sha256": "aa3a9b321f4b0bb2c40203ffbde9af89713227866e0e13f76e5b9eeea727cf88",
      "sharedFont": {
        "id": 16,
        "offset": 0
      }
    },
    {
      "key": "document-font-17-0.bin",
      "bytes": 888808,
      "sha256": "e618af7bf999df192ed4f388eba2e563f2b5015034e9cbb317b5bd793bd7334d",
      "sharedFont": {
        "id": 17,
        "offset": 0
      }
    },
    {
      "key": "document-font-18-0.bin",
      "bytes": 218652,
      "sha256": "5a1c559bb539583c8a1fd99d1c5b9491e5e14478c9cd2bd0970d5c3096cc9ef8",
      "sharedFont": {
        "id": 18,
        "offset": 0
      }
    },
    {
      "key": "document-font-19-0.bin",
      "bytes": 1048576,
      "sha256": "d84aa2e2627454cf611ed7b5b3a1cae302c693417d50d9bc80549f82799fd60d",
      "sharedFont": {
        "id": 19,
        "offset": 0
      }
    },
    {
      "key": "document-font-19-1.bin",
      "bytes": 1048576,
      "sha256": "eda189644dbbd2e6aa8b557fa498669a37b3eae16afc05868193ed56eb20634c",
      "sharedFont": {
        "id": 19,
        "offset": 1048576
      }
    },
    {
      "key": "document-font-19-2.bin",
      "bytes": 1048576,
      "sha256": "8a1d44c13a09dc3cee06917db3a71b0030de34b6877de8d6e68c116b4046b365",
      "sharedFont": {
        "id": 19,
        "offset": 2097152
      }
    },
    {
      "key": "document-font-19-3.bin",
      "bytes": 1048576,
      "sha256": "310da7871a17cd550f0a537f5b42fcb3128f572ead1cced7ccf5a8e92e03b67a",
      "sharedFont": {
        "id": 19,
        "offset": 3145728
      }
    },
    {
      "key": "document-font-19-4.bin",
      "bytes": 1048576,
      "sha256": "c84a61bd41a6a3d21c0f931bfc80ba01d0ec565cb9100eabb8881bd48d2a3a09",
      "sharedFont": {
        "id": 19,
        "offset": 4194304
      }
    },
    {
      "key": "document-font-19-5.bin",
      "bytes": 1048576,
      "sha256": "683687208e646b9b1e215766ad64d5918ce72a7c6ad9d6e40e0f59be3e6aadf5",
      "sharedFont": {
        "id": 19,
        "offset": 5242880
      }
    },
    {
      "key": "document-font-19-6.bin",
      "bytes": 1048576,
      "sha256": "e40558d3e02b86b1007406b1278b7a9560322c64ccf1500beef65d58405bdf27",
      "sharedFont": {
        "id": 19,
        "offset": 6291456
      }
    },
    {
      "key": "document-font-19-7.bin",
      "bytes": 1048576,
      "sha256": "57cfa074b535d427a837842b14979edbeaf9233d6e90b11b9d958f44cf147e66",
      "sharedFont": {
        "id": 19,
        "offset": 7340032
      }
    },
    {
      "key": "document-font-19-8.bin",
      "bytes": 1048576,
      "sha256": "1ac1f98f4050924e862898f6ad50e4df1751507d64dfb4ed3dac6c66165d2318",
      "sharedFont": {
        "id": 19,
        "offset": 8388608
      }
    },
    {
      "key": "document-font-19-9.bin",
      "bytes": 977404,
      "sha256": "ae649bfcab384075f0926cd9551d4757467921db5a95d234af5483cea3811809",
      "sharedFont": {
        "id": 19,
        "offset": 9437184
      }
    },
    {
      "key": "document-license-0",
      "path": "sdk/resources/document/license-0.txt",
      "bytes": 4473,
      "sha256": "bcb5f95766d5dc969748869eef4559dd991fd31f7e0d30c29e140276cc3c24ee",
      "url": new URL("../resources/document/license-0.txt",import.meta.url).href
    },
    {
      "key": "document-license-1",
      "path": "sdk/resources/document/license-1.txt",
      "bytes": 4388,
      "sha256": "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9",
      "url": new URL("../resources/document/license-1.txt",import.meta.url).href
    },
    {
      "key": "document-license-2",
      "path": "sdk/resources/document/license-2.txt",
      "bytes": 4382,
      "sha256": "07fc70bfeb985cc1a87a8587d0a0c80bab11c86c9dc3fd95b6f0cb332f983e96",
      "url": new URL("../resources/document/license-2.txt",import.meta.url).href
    },
    {
      "key": "document-license-3",
      "path": "sdk/resources/document/license-3.txt",
      "bytes": 4386,
      "sha256": "a216f6f8d85c7228093e0ee5e258d9d377e6671f68acb4db1930b29583d0f331",
      "url": new URL("../resources/document/license-3.txt",import.meta.url).href
    },
    {
      "key": "document-license-4",
      "path": "sdk/resources/document/license-4.txt",
      "bytes": 4383,
      "sha256": "754f0e221aa7d5a915489f3bf1f20fe53ddc35ab2834a4d91656d78f9622de70",
      "url": new URL("../resources/document/license-4.txt",import.meta.url).href
    },
    {
      "key": "document-license-5",
      "path": "sdk/resources/document/license-5.txt",
      "bytes": 4381,
      "sha256": "f8ff8ce7d0a81bf8d5e121c635ef027250c531f2fd37d5988b8dd6e45f19d7f1",
      "url": new URL("../resources/document/license-5.txt",import.meta.url).href
    },
    {
      "key": "document-license-6",
      "path": "sdk/resources/document/license-6.txt",
      "bytes": 4382,
      "sha256": "481c72a8f1b4f645a7e6b10326be41b2da2c15cb470ff48a07fab773eed00102",
      "url": new URL("../resources/document/license-6.txt",import.meta.url).href
    },
    {
      "key": "document-license-7",
      "path": "sdk/resources/document/license-7.txt",
      "bytes": 4380,
      "sha256": "2e98fd23a52d253db8612cd5942c8f2ff4111b21d2367050fdca91d8ccc374a0",
      "url": new URL("../resources/document/license-7.txt",import.meta.url).href
    },
    {
      "key": "document-license-8",
      "path": "sdk/resources/document/license-8.txt",
      "bytes": 4388,
      "sha256": "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9",
      "url": new URL("../resources/document/license-8.txt",import.meta.url).href
    }
  ]
}));
