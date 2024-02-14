# TODO
- Line graphics
- Cursor movement & line collision
- line animation
- Triangle sigils
- Level loading from TOML
- Fix dependencies
- Level editor

# Sigils
## Runes 
defines activity
uppercase = !lowercase;
- α = connected
- β = enclosed
- δ = loops
- ζ = selected

## Shape
defines lines
- circle = plain
- triangle = no entering
- square = can cross
- pentagon = all entering changes their endpoint to exiting
- hexagon = exiting disconnects from hexagon & connects to all entering hexagon

## Aura
defines cursors
- circle = clones
- triangle = destroys
- square = toggles case