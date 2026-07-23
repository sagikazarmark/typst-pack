#let phase = sys.inputs.at("phase")
#if phase == "creation" {
  read("resources/discovered.txt")
} else {
  assert(phase == "compilation")
}
#rect(width: 1pt, height: 1pt)
