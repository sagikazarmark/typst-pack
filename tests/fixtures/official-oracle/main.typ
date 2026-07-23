#import "@local/oracle:1.0.0": oracle-box
#import "chapter.typ": chapter-radius
#let width = int(sys.inputs.width)
#let _ = decimal(1.1)
#set page(width: 80pt, height: 40pt, margin: 0pt)
#oracle-box(width: width * 1pt)
#pagebreak()
#circle(radius: chapter-radius)
