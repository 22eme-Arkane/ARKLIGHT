// FICHIER GÉNÉRÉ par tools/ofl/import.mjs — ne pas modifier à la main.
// Profils DMX : Open Fixture Library (https://open-fixture-library.org), licence MIT.
// Importé le 2026-10-06. Sortie lumineuse estimée (output.estimated).
export const OFL_FIXTURES = [
 {
  "id": "arri.skypanel-s30c",
  "brand": "ARRI",
  "name": "Skypanel S30C",
  "model": "Skypanel S30C",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.499,
   "height": 0.133,
   "depth": 0.397
  },
  "pixels": 1,
  "weight": 8,
  "watts": 220,
  "beamAngle": 115,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 9900,
   "luxAt1m": 2895,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/skypanel-s30c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "arri.skypanel-s60c",
  "brand": "ARRI",
  "name": "Skypanel S60C",
  "model": "Skypanel S60C",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.688,
   "height": 0.347,
   "depth": 0.133
  },
  "pixels": 1,
  "weight": 12.8,
  "watts": 420,
  "beamAngle": 115,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 18900,
   "luxAt1m": 5526,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/skypanel-s60c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "arri.skypanel-s120c",
  "brand": "ARRI",
  "name": "Skypanel S120C",
  "model": "Skypanel S120C",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.332,
   "height": 0.347,
   "depth": 0.133
  },
  "pixels": 1,
  "weight": 13.7,
  "watts": 400,
  "beamAngle": 110,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 18000,
   "luxAt1m": 5710,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/skypanel-s120c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "arri.skypanel-s360c",
  "brand": "ARRI",
  "name": "Skypanel S360C",
  "model": "Skypanel S360C",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.331,
   "height": 0.947,
   "depth": 0.186
  },
  "pixels": 1,
  "weight": 41,
  "watts": 1500,
  "beamAngle": 105,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 67500,
   "luxAt1m": 23340,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.07,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      },
      {
       "label": "Fan Control",
       "f": "none"
      },
      {
       "label": "Presets",
       "f": "none"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 19,
        "min": 1,
        "max": 25
       }
      },
      {
       "label": "Reserved for future use",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/skypanel-s360c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "arri.l5-c",
  "brand": "ARRI",
  "name": "L5-C",
  "model": "L5-C",
  "category": "fresnel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.198,
   "depth": 0.373
  },
  "pixels": 1,
  "weight": 5.2,
  "watts": 115,
  "beamAngle": 32,
  "zoom": {
   "min": 14,
   "max": 50,
   "luxAt1m": [
    [
     14,
     49110
    ],
    [
     50,
     3907
    ]
   ]
  },
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 4600,
   "luxAt1m": 9449,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/l5-c"
  },
  "defaults": {
   "settings": {
    "zoom": 32
   }
  }
 },
 {
  "id": "arri.l7-c",
  "brand": "ARRI",
  "name": "L7-C",
  "model": "L7-C",
  "category": "fresnel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.27,
   "depth": 0.456
  },
  "pixels": 1,
  "weight": 8.2,
  "watts": 220,
  "beamAngle": 33,
  "zoom": {
   "min": 15,
   "max": 50,
   "luxAt1m": [
    [
     15,
     81855
    ],
    [
     50,
     7474
    ]
   ]
  },
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 8800,
   "luxAt1m": 17005,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/l7-c"
  },
  "defaults": {
   "settings": {
    "zoom": 33
   }
  }
 },
 {
  "id": "arri.l10-c",
  "brand": "ARRI",
  "name": "L10-C",
  "model": "L10-C",
  "category": "fresnel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.35550000000000004,
   "depth": 0.745
  },
  "pixels": 1,
  "weight": 19,
  "watts": 510,
  "beamAngle": 33,
  "zoom": {
   "min": 15,
   "max": 50,
   "luxAt1m": [
    [
     15,
     189755
    ],
    [
     50,
     17327
    ]
   ]
  },
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2800,
   10000
  ],
  "output": {
   "lumens": 20400,
   "luxAt1m": 39421,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P01: CCT & RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      }
     ]
    },
    {
     "id": "m2",
     "name": "P02: CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m3",
     "name": "P03: CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P04: RGBW 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P05: HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m6",
     "name": "P06: CCT & RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m7",
     "name": "P07: CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      }
     ]
    },
    {
     "id": "m8",
     "name": "P08: CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 16bit",
       "f": "tint",
       "neutral": 0.0763,
       "bytes": 2
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m9",
     "name": "P09: RGBW 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10: HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11: CCT & RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12: CCT 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13: CCT & HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        10000
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point 8bit",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14: RGBW 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      },
      {
       "label": "White",
       "f": "white",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15: HSI 8/16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/l10-c"
  },
  "defaults": {
   "settings": {
    "zoom": 33
   }
  }
 },
 {
  "id": "arri.broadcaster-2-plus",
  "brand": "ARRI",
  "name": "Broadcaster 2 Plus",
  "model": "Broadcaster 2 Plus",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.218,
   "height": 0.096,
   "depth": 0.07
  },
  "pixels": 1,
  "weight": 0.96,
  "watts": 35,
  "beamAngle": 63,
  "emitters": "bicolore 2800–6500 K",
  "cctRange": [
   2800,
   6500
  ],
  "output": {
   "lumens": 1575,
   "luxAt1m": 1446,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "4-channel",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2800,
        6500
       ]
      },
      {
       "label": "Plus/Minus Green",
       "f": "tint"
      },
      {
       "label": "Reserved",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/arri/broadcaster-2-plus"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.ls-600d-pro",
  "brand": "Aputure",
  "name": "LS 600D Pro",
  "model": "LS 600D Pro",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.1404,
   "depth": 0.15
  },
  "pixels": 1,
  "weight": 4.6,
  "watts": 600,
  "beamAngle": 55,
  "emitters": "lumière du jour",
  "output": {
   "lumens": 42000,
   "luxAt1m": 35496,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Lighting & Effect 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/ls-600d-pro"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.ls-600d",
  "brand": "Aputure",
  "name": "LS 600D",
  "model": "LS 600D",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.1404,
   "depth": 0.121
  },
  "pixels": 1,
  "weight": 4.7,
  "watts": 600,
  "beamAngle": 55,
  "emitters": "lumière du jour",
  "output": {
   "lumens": 42000,
   "luxAt1m": 35496,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Lighting 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      }
     ]
    },
    {
     "id": "m2",
     "name": "Effect 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "Lighting & Effect 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/ls-600d"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.ls-600x-pro",
  "brand": "Aputure",
  "name": "LS 600X Pro",
  "model": "LS 600X Pro",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.1323,
   "depth": 0.149
  },
  "pixels": 1,
  "weight": 5.2,
  "watts": 600,
  "beamAngle": 55,
  "emitters": "bicolore 2700–6500 K",
  "cctRange": [
   2700,
   6500
  ],
  "output": {
   "lumens": 42000,
   "luxAt1m": 35496,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Lighting 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        2700,
        6500
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "Effects 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        2700,
        6500
       ]
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "Lighting & Effects 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        2700,
        6500
       ]
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/ls-600x-pro"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.ls-1200d-pro",
  "brand": "Aputure",
  "name": "LS 1200D Pro",
  "model": "LS 1200D Pro",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.2079,
   "depth": 0.216
  },
  "pixels": 1,
  "weight": 7.6,
  "watts": 1200,
  "beamAngle": 55,
  "emitters": "lumière du jour",
  "output": {
   "lumens": 84000,
   "luxAt1m": 70993,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Lighting 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      }
     ]
    },
    {
     "id": "m2",
     "name": "Effect 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    },
    {
     "id": "m3",
     "name": "Lighting & Effect 8-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "Lighting 16-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m5",
     "name": "Effect 16-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "Lighting & Effect 16-bit",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "Effects",
       "f": "none"
      },
      {
       "label": "Frequency",
       "f": "none"
      },
      {
       "label": "Lightning",
       "f": "none"
      },
      {
       "label": "Explosion",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/ls-1200d-pro"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.ls-300x",
  "brand": "Aputure",
  "name": "LS 300x",
  "model": "LS 300x",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.1944,
   "depth": 0.345
  },
  "pixels": 1,
  "weight": 11.4,
  "watts": 300,
  "beamAngle": 55,
  "emitters": "bicolore 2700–6500 K",
  "cctRange": [
   2700,
   6500
  ],
  "output": {
   "lumens": 21000,
   "luxAt1m": 17748,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Default",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT Control",
       "f": "cct",
       "range": [
        2700,
        6500
       ]
      },
      {
       "label": "Frequency Control",
       "f": "none"
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "FX Control",
       "f": "none"
      },
      {
       "label": "Start Trigger",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/ls-300x"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.c300d",
  "brand": "Aputure",
  "name": "LS C300d",
  "model": "LS C300d",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.19404,
   "depth": 0.2984
  },
  "pixels": 1,
  "weight": 5,
  "watts": 350,
  "beamAngle": 55,
  "emitters": "lumière du jour",
  "output": {
   "lumens": 24500,
   "luxAt1m": 20706,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "5-channel",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Mode Selection",
       "f": "none"
      },
      {
       "label": "FX Control",
       "f": "none"
      },
      {
       "label": "FX Frequency Control",
       "f": "none"
      },
      {
       "f": "none",
       "label": "FX Options"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/c300d"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "aputure.nova-p300c",
  "brand": "Aputure",
  "name": "Nova P300c",
  "model": "Nova P300c",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.562,
   "height": 0.457,
   "depth": 0.166
  },
  "pixels": 1,
  "weight": 10.35,
  "watts": 300,
  "beamAngle": 110,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2000,
   10000
  ],
  "output": {
   "lumens": 13500,
   "luxAt1m": 4283,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "CCT&RGB",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        2000,
        10000
       ]
      },
      {
       "label": "Green/Magenta Point",
       "f": "tint"
      },
      {
       "label": "CCT <-> RGB",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m2",
     "name": "CCT",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        2000,
        10000
       ]
      },
      {
       "label": "Green/Magenta Point",
       "f": "tint"
      }
     ]
    },
    {
     "id": "m3",
     "name": "CCT&HSI",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        2000,
        10000
       ]
      },
      {
       "label": "Green/Magenta Point",
       "f": "tint"
      },
      {
       "label": "CCT <-> HSI",
       "f": "crossfade"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "dimmer"
      }
     ]
    },
    {
     "id": "m4",
     "name": "GEL",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Gel CCT",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Gel R/L",
       "f": "none"
      },
      {
       "label": "Gel Series",
       "f": "none"
      },
      {
       "label": "Gel Model",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "HSI",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "dimmer"
      }
     ]
    },
    {
     "id": "m6",
     "name": "Source",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Source",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "RGB",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m8",
     "name": "X-Y",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "X",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m9",
     "name": "Effect",
     "channels": [
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "Effect Parameter 1",
       "f": "none"
      },
      {
       "label": "Effect Parameter 2",
       "f": "none"
      },
      {
       "label": "Effect Parameter 3",
       "f": "none"
      },
      {
       "label": "Effect Parameter 4",
       "f": "none"
      },
      {
       "label": "Effect Parameter 5",
       "f": "none"
      },
      {
       "label": "Effect Parameter 6",
       "f": "none"
      },
      {
       "label": "Effect Parameter 7",
       "f": "none"
      },
      {
       "label": "Effect Parameter 8",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/aputure/nova-p300c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "dedolight.dled4-bi",
  "brand": "Dedolight",
  "name": "DLED4-BI",
  "model": "DLED4-BI",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.0981,
   "depth": 0.1
  },
  "pixels": 1,
  "weight": 1.3,
  "watts": 47,
  "beamAngle": 32,
  "zoom": {
   "min": 4,
   "max": 60,
   "luxAt1m": [
    [
     4,
     515735
    ],
    [
     60,
     2345
    ]
   ]
  },
  "emitters": "bicolore 2600–6800 K",
  "cctRange": [
   2600,
   6800
  ],
  "output": {
   "lumens": 3290,
   "luxAt1m": 8110,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "DT4-BI-DMX",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6800
       ]
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/dedolight/dled4-bi"
  },
  "defaults": {
   "settings": {
    "zoom": 32
   }
  }
 },
 {
  "id": "dedolight.dled7-bi",
  "brand": "Dedolight",
  "name": "DLED7-BI",
  "model": "DLED7-BI",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.0981,
   "depth": 0.1
  },
  "pixels": 1,
  "weight": 1.3,
  "watts": 90,
  "beamAngle": 32,
  "zoom": {
   "min": 4,
   "max": 60,
   "luxAt1m": [
    [
     4,
     987578
    ],
    [
     60,
     4490
    ]
   ]
  },
  "emitters": "bicolore 2700–6500 K",
  "cctRange": [
   2700,
   6500
  ],
  "output": {
   "lumens": 6300,
   "luxAt1m": 15530,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "DT7-BI-DMX",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2700,
        6500
       ]
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/dedolight/dled7-bi"
  },
  "defaults": {
   "settings": {
    "zoom": 32
   }
  }
 },
 {
  "id": "dmg-lumiere.sl1-mix",
  "brand": "DMG Lumière",
  "name": "SL1 MIX",
  "model": "SL1 MIX",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.118,
   "height": 0.205,
   "depth": 0.037
  },
  "pixels": 1,
  "weight": 4.18,
  "watts": 200,
  "beamAngle": 110,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   1700,
   10000
  ],
  "output": {
   "lumens": 9000,
   "luxAt1m": 2855,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "1: White 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m2",
     "name": "2: Color 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m3",
     "name": "3: Gel 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "4: Pro 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      },
      {
       "label": "Effects Speed",
       "f": "none"
      },
      {
       "label": "Colorspace X Position",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Colorspace Y Position",
       "f": "y",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "5: Pro 16-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      },
      {
       "label": "Effects Speed",
       "f": "none"
      },
      {
       "label": "Colorspace X Position",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Colorspace Y Position",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "6: Full 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Amber",
       "f": "amber"
      },
      {
       "label": "Lime",
       "f": "lime"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "7: Full 16-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Amber",
       "f": "amber"
      },
      {
       "label": "Lime",
       "f": "lime"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/dmg-lumiere/sl1-mix"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "dmg-lumiere.maxi-mix",
  "brand": "DMG Lumière",
  "name": "MAXI MIX",
  "model": "MAXI MIX",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.2,
   "height": 0.36,
   "depth": 0.085
  },
  "pixels": 1,
  "weight": 8,
  "watts": 360,
  "beamAngle": 110,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   1700,
   10000
  ],
  "output": {
   "lumens": 16200,
   "luxAt1m": 5139,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "1: White 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m2",
     "name": "2: Color 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m3",
     "name": "3: Gel 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "4: Pro 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      },
      {
       "label": "Effects Speed",
       "f": "none"
      },
      {
       "label": "Colorspace X Position",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Colorspace Y Position",
       "f": "y",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "5: Pro 16-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      },
      {
       "label": "Effects Speed",
       "f": "none"
      },
      {
       "label": "Colorspace X Position",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Colorspace Y Position",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "6: Full 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Amber",
       "f": "amber"
      },
      {
       "label": "Lime",
       "f": "lime"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "7: Full 16-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Amber",
       "f": "amber"
      },
      {
       "label": "Lime",
       "f": "lime"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/dmg-lumiere/maxi-mix"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "dmg-lumiere.mini-mix",
  "brand": "DMG Lumière",
  "name": "MINI MIX",
  "model": "MINI MIX",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.585,
   "height": 0.205,
   "depth": 0.037
  },
  "pixels": 1,
  "weight": 2.56,
  "watts": 100,
  "beamAngle": 110,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   1700,
   10000
  ],
  "output": {
   "lumens": 4500,
   "luxAt1m": 1428,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "1: White 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m2",
     "name": "2: Color 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m3",
     "name": "3: Gel 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m4",
     "name": "4: Pro 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      },
      {
       "label": "Effects Speed",
       "f": "none"
      },
      {
       "label": "Colorspace X Position",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Colorspace Y Position",
       "f": "y",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "5: Pro 16-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Memory # Hundreds",
       "f": "none"
      },
      {
       "label": "Memory # Tens",
       "f": "none"
      },
      {
       "label": "Memory # Singles",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "White Base Color",
       "f": "cct",
       "range": [
        3200,
        5600
       ]
      },
      {
       "label": "Hue Gel",
       "f": "none"
      },
      {
       "label": "Saturation Gel",
       "f": "none"
      },
      {
       "label": "Effects Speed",
       "f": "none"
      },
      {
       "label": "Colorspace X Position",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Colorspace Y Position",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m6",
     "name": "6: Full 8-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Amber",
       "f": "amber"
      },
      {
       "label": "Lime",
       "f": "lime"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    },
    {
     "id": "m7",
     "name": "7: Full 16-bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        1700,
        10000
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Cross Fade to Color",
       "f": "crossfade"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      },
      {
       "label": "White",
       "f": "white"
      },
      {
       "label": "Amber",
       "f": "amber"
      },
      {
       "label": "Lime",
       "f": "lime"
      },
      {
       "label": "Strobe",
       "f": "strobe",
       "strobe": {
        "open": 16,
        "min": 1,
        "max": 17.8
       }
      },
      {
       "label": "Control",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/dmg-lumiere/mini-mix"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "kino-flo.celeb-250-led-dmx",
  "brand": "Kino Flo",
  "name": "Celeb 250 LED DMX",
  "model": "Celeb 250 LED DMX",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.61,
   "height": 0.36,
   "depth": 0.13
  },
  "pixels": 1,
  "weight": 6.8,
  "watts": 144,
  "beamAngle": 100,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2500,
   9900
  ],
  "output": {
   "lumens": 6480,
   "luxAt1m": 2454,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P1 DIMMING & CCT",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "P2 HSI + CCT, Gel 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Gel",
       "f": "none"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m3",
     "name": "P3 CCT, RGB 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P4 FX 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P5 CIE xy 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m6",
     "name": "P6 CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m7",
     "name": "P7 HSI & CCT, Gel 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Gel",
       "f": "none"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m8",
     "name": "P8 CCT, RGB 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m9",
     "name": "P9 FX 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10 CIE xy 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11 CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12 CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13 Gel 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14 Gel 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15 HSI & CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m16",
     "name": "P16 HSI & CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m17",
     "name": "P17 RGB & CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m18",
     "name": "P18 RGB & CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m19",
     "name": "P19 CIE xy 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m20",
     "name": "P20 CIE xy 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      }
     ]
    },
    {
     "id": "m21",
     "name": "P21 FX 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m22",
     "name": "P22 FX 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m23",
     "name": "P23 xfade CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m24",
     "name": "P24 xfade CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m25",
     "name": "P25 xfade CCT & RBG 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m26",
     "name": "P26 xfade CCT & RBG 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m27",
     "name": "P27 xfade CIE xy1 & CIE xy2 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "CIE x2",
       "f": "none"
      },
      {
       "label": "CIE y2",
       "f": "none"
      }
     ]
    },
    {
     "id": "m28",
     "name": "P28 xfade CIE xy1 & CIE xy2 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "CIE x2",
       "f": "none",
       "bytes": 2
      },
      {
       "label": "CIE y2",
       "f": "none",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m29",
     "name": "P29 xfade CCT & TDRGB 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "Tungsten",
       "f": "dimmer"
      },
      {
       "label": "Daylight",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m30",
     "name": "P30 xfade CCT & TDRGB 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Tungsten",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Daylight",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/kino-flo/celeb-250-led-dmx"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "kino-flo.celeb-450-led-dmx",
  "brand": "Kino Flo",
  "name": "Celeb 450 LED DMX",
  "model": "Celeb 450 LED DMX",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.145,
   "height": 0.36,
   "depth": 0.13
  },
  "pixels": 1,
  "weight": 12,
  "watts": 300,
  "beamAngle": 100,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   2500,
   9900
  ],
  "output": {
   "lumens": 13500,
   "luxAt1m": 5113,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "P1 DIMMING & CCT",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "P2 HSI + CCT, Gel 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Gel",
       "f": "none"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m3",
     "name": "P3 CCT, RGB 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m4",
     "name": "P4 FX 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m5",
     "name": "P5 CIE xy 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m6",
     "name": "P6 CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m7",
     "name": "P7 HSI & CCT, Gel 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Gel",
       "f": "none"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m8",
     "name": "P8 CCT, RGB 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m9",
     "name": "P9 FX 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m10",
     "name": "P10 CIE xy 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m11",
     "name": "P11 CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m12",
     "name": "P12 CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      }
     ]
    },
    {
     "id": "m13",
     "name": "P13 Gel 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m14",
     "name": "P14 Gel 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Gel",
       "f": "none"
      }
     ]
    },
    {
     "id": "m15",
     "name": "P15 HSI & CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m16",
     "name": "P16 HSI & CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m17",
     "name": "P17 RGB & CCT 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m18",
     "name": "P18 RGB & CCT 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m19",
     "name": "P19 CIE xy 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      }
     ]
    },
    {
     "id": "m20",
     "name": "P20 CIE xy 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      }
     ]
    },
    {
     "id": "m21",
     "name": "P21 FX 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m22",
     "name": "P22 FX 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Effect",
       "f": "none"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "FX Param 1",
       "f": "none"
      },
      {
       "label": "FX Param 2",
       "f": "none"
      },
      {
       "label": "FX Param 3",
       "f": "none"
      },
      {
       "label": "FX Param 4",
       "f": "none"
      },
      {
       "label": "FX Param 5",
       "f": "none"
      }
     ]
    },
    {
     "id": "m23",
     "name": "P23 xfade CCT & HSI 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Saturation",
       "f": "sat"
      }
     ]
    },
    {
     "id": "m24",
     "name": "P24 xfade CCT & HSI 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2",
       "bytes": 2
      },
      {
       "label": "Hue",
       "f": "hue",
       "bytes": 2
      },
      {
       "label": "Saturation",
       "f": "sat",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m25",
     "name": "P25 xfade CCT & RBG 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m26",
     "name": "P26 xfade CCT & RBG 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "slot": "2",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m27",
     "name": "P27 xfade CIE xy1 & CIE xy2 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ]
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "CIE x2",
       "f": "none"
      },
      {
       "label": "CIE y2",
       "f": "none"
      }
     ]
    },
    {
     "id": "m28",
     "name": "P28 xfade CIE xy1 & CIE xy2 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "CIE x",
       "f": "x",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "CIE y",
       "f": "y",
       "range": [
        0,
        1
       ],
       "bytes": 2
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "CIE x2",
       "f": "none",
       "bytes": 2
      },
      {
       "label": "CIE y2",
       "f": "none",
       "bytes": 2
      }
     ]
    },
    {
     "id": "m29",
     "name": "P29 xfade CCT & TDRGB 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ]
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade"
      },
      {
       "label": "Tungsten",
       "f": "dimmer"
      },
      {
       "label": "Daylight",
       "f": "dimmer"
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m30",
     "name": "P30 xfade CCT & TDRGB 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2500,
        9900
       ],
       "bytes": 2
      },
      {
       "label": "Green-Magenta Point",
       "f": "tint",
       "neutral": 0.0392
      },
      {
       "label": "Crossfade",
       "f": "crossfade",
       "bytes": 2
      },
      {
       "label": "Tungsten",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Daylight",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Red",
       "f": "red",
       "bytes": 2
      },
      {
       "label": "Green",
       "f": "green",
       "bytes": 2
      },
      {
       "label": "Blue",
       "f": "blue",
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/kino-flo/celeb-450-led-dmx"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "kino-flo.celeb-led-201-dmx",
  "brand": "Kino Flo",
  "name": "Celeb LED 201 DMX",
  "model": "Celeb LED 201 DMX",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.61,
   "height": 0.36,
   "depth": 0.13
  },
  "pixels": 1,
  "weight": 6.8,
  "watts": 100,
  "beamAngle": 100,
  "emitters": "bicolore 2700–6500 K",
  "cctRange": [
   2700,
   6500
  ],
  "output": {
   "lumens": 4500,
   "luxAt1m": 1704,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "DIMMING + CCT",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2700,
        6500
       ]
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/kino-flo/celeb-led-201-dmx"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "litegear.litemat-plus-2",
  "brand": "LiteGear",
  "name": "LiteMat Plus 2",
  "model": "LiteMat Plus 2",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.533,
   "height": 0.533,
   "depth": 0.023
  },
  "pixels": 2,
  "pixelLayout": {
   "cols": 2,
   "rows": 1
  },
  "weight": 1.72,
  "watts": 100,
  "beamAngle": 120,
  "emitters": "bicolore 2600–6200 K",
  "cctRange": [
   2600,
   6200
  ],
  "output": {
   "lumens": 5096,
   "luxAt1m": 1379,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Single 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "Single 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "bytes": 2
      }
     ]
    },
    {
     "id": "m3",
     "name": "Double 8bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 0
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 1
      }
     ]
    },
    {
     "id": "m4",
     "name": "Double 16bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1,
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 1,
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/litegear/litemat-plus-2"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "litegear.litemat-plus-4",
  "brand": "LiteGear",
  "name": "LiteMat Plus 4",
  "model": "LiteMat Plus 4",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.016,
   "height": 0.533,
   "depth": 0.023
  },
  "pixels": 2,
  "pixelLayout": {
   "cols": 2,
   "rows": 1
  },
  "weight": 2.81,
  "watts": 200,
  "beamAngle": 120,
  "emitters": "bicolore 2600–6200 K",
  "cctRange": [
   2600,
   6200
  ],
  "output": {
   "lumens": 10192,
   "luxAt1m": 2758,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Single 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "Single 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "bytes": 2
      }
     ]
    },
    {
     "id": "m3",
     "name": "Double 8bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 0
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 1
      }
     ]
    },
    {
     "id": "m4",
     "name": "Double 16bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1,
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6200
       ],
       "px": 1,
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/litegear/litemat-plus-4"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "litegear.s2-litemat-2",
  "brand": "LiteGear",
  "name": "S2 LiteMat 2",
  "model": "S2 LiteMat 2",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 0.533,
   "height": 0.533,
   "depth": 0.023
  },
  "pixels": 2,
  "pixelLayout": {
   "cols": 2,
   "rows": 1
  },
  "weight": 1.6,
  "watts": 100,
  "beamAngle": 120,
  "emitters": "bicolore 2600–6000 K",
  "cctRange": [
   2600,
   6000
  ],
  "output": {
   "lumens": 5470,
   "luxAt1m": 1480,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Single 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "Single 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "bytes": 2
      }
     ]
    },
    {
     "id": "m3",
     "name": "Double 8bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 0
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 1
      }
     ]
    },
    {
     "id": "m4",
     "name": "Double 16bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1,
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 1,
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/litegear/s2-litemat-2"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "litegear.s2-litemat-4",
  "brand": "LiteGear",
  "name": "S2 LiteMat 4",
  "model": "S2 LiteMat 4",
  "category": "panel",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "panel",
   "width": 1.016,
   "height": 0.533,
   "depth": 0.023
  },
  "pixels": 2,
  "pixelLayout": {
   "cols": 2,
   "rows": 1
  },
  "weight": 2.6,
  "watts": 200,
  "beamAngle": 120,
  "emitters": "bicolore 2600–6000 K",
  "cctRange": [
   2600,
   6000
  ],
  "output": {
   "lumens": 10940,
   "luxAt1m": 2960,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "Single 8bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer"
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ]
      }
     ]
    },
    {
     "id": "m2",
     "name": "Single 16bit",
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "bytes": 2
      }
     ]
    },
    {
     "id": "m3",
     "name": "Double 8bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 0
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 1
      }
     ]
    },
    {
     "id": "m4",
     "name": "Double 16bit",
     "pixels": 2,
     "channels": [
      {
       "label": "Dimmer",
       "f": "dimmer",
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Color Temperature",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 0,
       "bytes": 2
      },
      {
       "label": "Dimmer 2",
       "f": "dimmer",
       "px": 1,
       "bytes": 2
      },
      {
       "label": "Color Temperature 2",
       "f": "cct",
       "range": [
        2600,
        6000
       ],
       "px": 1,
       "bytes": 2
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/litegear/s2-litemat-4"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "hive.wasp-100-c",
  "brand": "Hive Lighting",
  "name": "Wasp 100-C",
  "model": "Wasp 100-C",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.09000000000000001,
   "depth": 0.199
  },
  "pixels": 1,
  "weight": 1.1,
  "watts": 75,
  "beamAngle": 40,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   1650,
   8025
  ],
  "output": {
   "lumens": 5250,
   "luxAt1m": 8313,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "HSI",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Hue",
       "f": "hue"
      }
     ]
    },
    {
     "id": "m2",
     "name": "RGB+CCT",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m3",
     "name": "FX+",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Sources",
       "f": "none"
      },
      {
       "label": "Corrective",
       "f": "none"
      },
      {
       "label": "EFX",
       "f": "none"
      },
      {
       "label": "EFX Speed",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/hive/wasp-100-c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "hive.hornet-200-c",
  "brand": "Hive Lighting",
  "name": "Hornet 200-C",
  "model": "Hornet 200-C",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.09000000000000001,
   "depth": 0.224
  },
  "pixels": 1,
  "weight": 2.6,
  "watts": 150,
  "beamAngle": 40,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   1650,
   8025
  ],
  "output": {
   "lumens": 10500,
   "luxAt1m": 16626,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "HSI",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Hue",
       "f": "hue"
      }
     ]
    },
    {
     "id": "m2",
     "name": "RGB+CCT",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m3",
     "name": "FX+",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Sources",
       "f": "none"
      },
      {
       "label": "Corrective",
       "f": "none"
      },
      {
       "label": "EFX",
       "f": "none"
      },
      {
       "label": "EFX Speed",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/hive/hornet-200-c"
  },
  "defaults": {
   "settings": {}
  }
 },
 {
  "id": "hive.super-hornet-575-c",
  "brand": "Hive Lighting",
  "name": "Super Hornet 575-C",
  "model": "Super Hornet 575-C",
  "category": "spot",
  "platform": "generic",
  "technology": "led",
  "geometry": {
   "kind": "par",
   "diameter": 0.09000000000000001,
   "depth": 0.305
  },
  "pixels": 1,
  "weight": 6.9,
  "watts": 600,
  "beamAngle": 40,
  "emitters": "couleur (RGB/RGBW…)",
  "cctRange": [
   1650,
   8025
  ],
  "output": {
   "lumens": 42000,
   "luxAt1m": 66504,
   "estimated": true
  },
  "dmx": {
   "modes": [
    {
     "id": "m1",
     "name": "HSI",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Hue",
       "f": "hue"
      }
     ]
    },
    {
     "id": "m2",
     "name": "RGB+CCT",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Red",
       "f": "red"
      },
      {
       "label": "Green",
       "f": "green"
      },
      {
       "label": "Blue",
       "f": "blue"
      }
     ]
    },
    {
     "id": "m3",
     "name": "FX+",
     "channels": [
      {
       "label": "Intensity",
       "f": "dimmer"
      },
      {
       "label": "CCT",
       "f": "cct",
       "range": [
        1650,
        8025
       ]
      },
      {
       "label": "Saturation",
       "f": "sat"
      },
      {
       "label": "Hue",
       "f": "hue"
      },
      {
       "label": "Sources",
       "f": "none"
      },
      {
       "label": "Corrective",
       "f": "none"
      },
      {
       "label": "EFX",
       "f": "none"
      },
      {
       "label": "EFX Speed",
       "f": "none"
      }
     ]
    }
   ]
  },
  "source": {
   "kind": "ofl",
   "url": "https://open-fixture-library.org/hive/super-hornet-575-c"
  },
  "defaults": {
   "settings": {}
  }
 }
];
