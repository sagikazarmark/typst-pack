#import "@local/greet:0.1.0": greet
#set page(width: 4cm, height: 2cm, margin: 2mm)
#assert(sys.inputs.at("mode", default: "creation") == "creation")
#greet("baseline")
#pagebreak()
Second baseline page.
