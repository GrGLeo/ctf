package game



type Flag struct {
	TeamId     Cell  `json:"teamID"`
	PosX       int  `json:"posx"`
	PosY       int  `json:"posy"`
	baseX      int  
	baseY      int  
	IsCaptured bool `json:"is_captured"`
}

func (f *Flag) Move(x, y int, board *Board) {
  board.Tracker.SaveDelta(f.PosX, f.PosY, Empty)
	f.PosX = x
	f.PosY = y
  board.Tracker.SaveDelta(f.PosX, f.PosY, f.TeamId)
}

func (f *Flag) SetBase() {
  f.baseX = f.PosX
  f.baseY = f.PosY
}

// SetBase need to be called first
func (f *Flag) ResetPos() {
  f.PosX = f.baseX
  f.PosY = f.baseY
  f.IsCaptured = false
}

// Return base flag position Y and X coordinate
func (f *Flag) GetBase() (int, int) {
  return f.baseY, f.baseX
}

// Check if flag is at his base position and is not captured
func (f *Flag) IsSafe() bool {
  return f.baseX == f.PosX && f.baseY == f.PosY && !f.IsCaptured
}

type WallPosition struct {
	StartPos [2]int // Y and X start position on the board
	EndPos   [2]int // Y and X end position on the board
}

// Return Y and X start position
func (w WallPosition) GetStartPos() (int, int) {
	return w.StartPos[0], w.StartPos[1]
}

// Return Y and X end position
func (w WallPosition) GetEndPos() (int, int) {
	return w.EndPos[0], w.EndPos[1]
}
