package model

import (
	"fmt"
	"log"
	"net"
	"strconv"
	"strings"
	"time"

	"github.com/GrGLeo/ctf/client/communication"
	"github.com/charmbracelet/bubbles/progress"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

type GameModel struct {
	currentBoard   [21][51]int
	conn           *net.TCPConn
	gameClock      time.Duration
	height, width  int
	healthProgress progress.Model
	progress       progress.Model
	health         [2]int
	points         [2]int
	dashed         bool
	dashcooldown   time.Duration
	dashStart      time.Time
	percent        float64
}

func NewGameModel(conn *net.TCPConn) GameModel {
	yellowGradient := progress.WithGradient(
		"#FFFF00", // Bright yellow
		"#FFD700", // Gold
	)
  redSolid := progress.WithSolidFill("#AB2C0F")
	return GameModel{
		conn:           conn,
		healthProgress: progress.New(redSolid),
		progress:       progress.New(yellowGradient),
		dashcooldown:   5 * time.Second,
	}
}

func (m GameModel) Init() tea.Cmd {
	return nil
}

func (m *GameModel) SetDimension(height, width int) {
	m.height = height
	m.width = width
	m.progress.Width = 51
}

func (m *GameModel) SetConnection(conn *net.TCPConn) {
	m.conn = conn
}

func (m GameModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case communication.BoardMsg:
		points := msg.Points
		m.points = points
		m.health = msg.Health
		m.currentBoard = msg.Board
	case communication.DeltaMsg:
		m.gameClock = time.Duration(50*int(msg.TickID)) * time.Millisecond
		points := msg.Points
		m.points = points
		ApplyDeltas(msg.Deltas, &m.currentBoard)
		return m, nil
	case tea.KeyMsg:
		switch msg.Type {
		case tea.KeyCtrlC, tea.KeyEsc:
			return m, tea.Quit
		}
		switch msg.String() {
		case "w":
			communication.SendAction(m.conn, 1)
			return m, nil
		case "s":
			communication.SendAction(m.conn, 2)
			return m, nil
		case "a":
			communication.SendAction(m.conn, 3)
			return m, nil
		case "d":
			communication.SendAction(m.conn, 4)
			return m, nil
		case " ":
			if !m.dashed {
				communication.SendAction(m.conn, 5)
				m.dashed = true
				m.dashStart = time.Now()
				return m, doTick()
			}
		case "j":
			communication.SendAction(m.conn, 6)
			return m, nil
		}
	case communication.CooldownTickMsg:
		var percent float64
		if m.dashed {
			elapsed := time.Since(m.dashStart)
			percent = float64(elapsed) / float64(m.dashcooldown)
			if percent >= 1.0 {
				percent = 0
				m.dashed = false
			}
		}
		m.percent = percent
		return m, doTick()
	}
	return m, nil
}

func (m GameModel) View() string {
	log.Println(m.points)
	log.Printf("Player Health: %d | %d\n", m.health[0], m.health[1])
	// Define styles
	bgStyle := lipgloss.NewStyle().Background(lipgloss.Color("0"))
	p1Style := lipgloss.NewStyle().Background(lipgloss.Color("21"))
	TowerDest := lipgloss.NewStyle().Background(lipgloss.Color("91"))
	bushStyle := lipgloss.NewStyle().Background(lipgloss.Color("34"))
	p4Style := lipgloss.NewStyle().Background(lipgloss.Color("220"))
	grayStyle := lipgloss.NewStyle().Background(lipgloss.Color("240"))
	Flag1Style := lipgloss.NewStyle().Background(lipgloss.Color("201"))
	TowerStyle := lipgloss.NewStyle().Background(lipgloss.Color("94"))
	FreezeStyle := lipgloss.NewStyle().Background(lipgloss.Color("105"))

	BluePointStyle := lipgloss.NewStyle().Background(lipgloss.Color("255")).Foreground(lipgloss.Color("21"))
	RedPointStyle := lipgloss.NewStyle().Background(lipgloss.Color("255")).Foreground(lipgloss.Color("34"))
	HudStyle := lipgloss.NewStyle().Background(lipgloss.Color("255")).Foreground(lipgloss.Color("0"))

	var builder strings.Builder

	// Construct score board
	bluePoints := strconv.Itoa(m.points[0])
	redPoints := strconv.Itoa(m.points[1])
	blueStr := BluePointStyle.Render(bluePoints)
	redStr := RedPointStyle.Render(redPoints)
	splitStr := HudStyle.Render(" | ")
	scoreText := HudStyle.Render(blueStr + splitStr + redStr)

	minutes := int(m.gameClock.Minutes())
	seconds := int(m.gameClock.Seconds()) % 60
	clockStr := HudStyle.Render(fmt.Sprintf("%02d:%02d", minutes, seconds))

	hud := lipgloss.Place(
		46,
		1,
		lipgloss.Center,
		lipgloss.Center,
		scoreText,
		lipgloss.WithWhitespaceChars(" "),
		lipgloss.WithWhitespaceBackground(HudStyle.GetBackground()),
	)

	hudContent := lipgloss.JoinHorizontal(lipgloss.Right, hud, clockStr)
	hudContent += "\n"
	builder.WriteString(hudContent)

	// Iterate through the board and apply styles
	for _, row := range m.currentBoard {
		for _, cell := range row {
			switch cell {
			case 0:
				builder.WriteString(grayStyle.Render(" ")) // Render gray for walls
			case 1:
				builder.WriteString(bgStyle.Render(" ")) // Render empty space for 1
			case 2:
				builder.WriteString(bushStyle.Render(" ")) // Render green for bush
			case 3:
				builder.WriteString(TowerDest.Render(" ")) // Render blue for player2
			case 4:
				builder.WriteString(p1Style.Render(" ")) // Render blue for player1
			case 5:
				builder.WriteString(p4Style.Render(" ")) // Render blue for player4
			case 6:
				builder.WriteString(Flag1Style.Render(" ")) // Render for flag1
			case 7:
				builder.WriteString(TowerStyle.Render(" ")) // Render for tower
			case 8:
				builder.WriteString(bgStyle.Render("⣿")) // Render for dash
			case 9:
				builder.WriteString(bgStyle.Render("x")) // Render for dash
			case 10:
				builder.WriteString(bgStyle.Render("▘")) // Render for dash
			case 11:
				builder.WriteString(bgStyle.Render("⣀")) // Render for dash
			case 12:
				builder.WriteString(FreezeStyle.Render("x")) // Render for freezing spell
			}
		}
		builder.WriteString("\n") // New line at the end of each row
	}

	var healthBar string
	if m.health[1] > 0 {
		healthPercent := (float32(m.health[0]) / float32(m.health[1]))
		healthBar = m.healthProgress.ViewAs(float64(healthPercent))
	}
	healthInfo := fmt.Sprintf("%d / %d", m.health[0], m.health[1])
	healthHUD := lipgloss.JoinHorizontal(
		lipgloss.Right,
		healthInfo,
		healthBar,
	)
	builder.WriteString(healthHUD)
	builder.WriteString("\n")

	var progressBar string
	if m.percent != 0.0 {
		progressBar = m.progress.ViewAs(m.percent)
	}
	builder.WriteString(progressBar)

	return lipgloss.Place(
		m.width,
		m.height,
		lipgloss.Center,
		lipgloss.Center,
		builder.String(),
	)
}

func doTick() tea.Cmd {
	return tea.Tick(50*time.Millisecond, func(time.Time) tea.Msg {
		return communication.CooldownTickMsg{}
	})
}

func ApplyDeltas(deltas [][3]int, currentBoard *[21][51]int) {
	for _, delta := range deltas {
		x := delta[0]
		y := delta[1]
		value := delta[2]
		currentBoard[y][x] = value
	}
}
