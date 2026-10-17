#import "@local/greet:0.1.0": greet
#let width = if "width" in sys.inputs { float(str(sys.inputs.width)) } else { 4 }
#let mode = sys.inputs.at("mode", default: "creation")
#assert(mode in ("creation", "compilation"))
#set page(width: width * 1cm, height: 2cm, margin: 2mm)
#greet(mode)
#pagebreak()
Second page.
