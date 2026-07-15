#let brand = read("branding/name.txt").trim()
#let detail = if brand == "First" {
  read("branding/first-only.txt").trim()
} else {
  read("branding/second-only.txt").trim()
}

#set page(width: 10cm, height: 4cm)
Brand: #brand (#detail)
