#let phase = sys.inputs.at("phase")
#if phase == "discovery" {
  read("discovered.txt")
} else {
  assert(phase == "compilation")
}
#rect(width: 1pt, height: 1pt)
