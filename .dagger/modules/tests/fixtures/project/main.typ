#import "@local/greet:0.1.0": greet
#set page(width: 10cm, height: 8cm)
#include "chapters/intro.typ"
#greet("Dagger")
Rows: #csv("data.csv").len()
