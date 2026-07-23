#import "@local/greet:0.1.0": greet
#set page(width: 5cm, height: 2cm, margin: 2mm)
#assert(sys.inputs.at("mode") == "compilation")
#greet("override")
#pagebreak()
Second overridden page.
