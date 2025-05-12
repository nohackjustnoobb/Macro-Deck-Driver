import { Component } from "react";
import "./App.css";

interface ModalData {
  idx: number | null;
  icon: string | null;
  command: string;
  args: string;
}

interface Button {
  icon: string | null;
  command: string | null;
  args: Array<string> | null;
}

type ButtonGrid = Button[];

interface AppState {
  rows: number;
  cols: number;
  inputRows: number;
  inputCols: number;
  grid: ButtonGrid;
  modalData: ModalData;
  isModalOpen: boolean;
  isModalClosing: boolean;
}

class App extends Component<object, AppState> {
  constructor(props: object) {
    super(props);
    this.state = {
      rows: 0,
      cols: 0,
      inputRows: 0,
      inputCols: 0,
      grid: [],
      modalData: {
        idx: null,
        icon: null,
        command: "",
        args: "",
      },
      isModalOpen: false,
      isModalClosing: false,
    };
  }

  private tooltipElement: HTMLDivElement | null = null;
  private activeTooltipIdx: number | null = null;

  handleCellClick = (idx: number) => {
    this.setState({
      modalData: {
        idx,
        icon: this.state.grid[idx].icon,
        command: this.state.grid[idx].command || "",
        args: this.state.grid[idx].args?.join(" ") || "",
      },
      isModalOpen: true,
      isModalClosing: false,
    });
  };

  private handleMouseMove = (e: MouseEvent) => {
    if (this.tooltipElement) {
      this.tooltipElement.style.left = `${e.clientX + 10}px`;
      this.tooltipElement.style.top = `${e.clientY + 10}px`;
    }
  };

  handleCellMouseEnter = (
    idx: number,
    event: React.MouseEvent<HTMLDivElement>
  ) => {
    if (this.tooltipElement && this.activeTooltipIdx !== idx) {
      this.handleCellMouseLeave();
    }

    if (this.activeTooltipIdx === idx && this.tooltipElement) {
      this.tooltipElement.style.left = `${event.clientX + 10}px`;
      this.tooltipElement.style.top = `${event.clientY + 10}px`;
      return;
    }

    const cell = this.state.grid[idx];
    const commandText = cell.command || "N/A";
    const validArgs = cell.args?.filter((arg) => arg.trim() !== "") || [];
    const argsText = validArgs.join(" ");
    const hasCommand = cell.command && cell.command.trim() !== "";
    const hasMeaningfulArgs = validArgs.length > 0;

    if (hasCommand || hasMeaningfulArgs) {
      this.tooltipElement = document.createElement("div");
      this.tooltipElement.className = "tooltip";
      this.tooltipElement.innerText = `Command: ${commandText}\nArgs: ${argsText}`;

      this.tooltipElement.style.position = "fixed";
      this.tooltipElement.style.zIndex = "1000";
      this.tooltipElement.style.pointerEvents = "none";
      this.tooltipElement.style.left = `${event.clientX + 10}px`;
      this.tooltipElement.style.top = `${event.clientY + 10}px`;

      document.body.appendChild(this.tooltipElement);
      document.addEventListener("mousemove", this.handleMouseMove);
      this.activeTooltipIdx = idx;
    }
  };

  handleCellMouseLeave = () => {
    if (this.tooltipElement) {
      if (this.tooltipElement.parentNode === document.body) {
        document.body.removeChild(this.tooltipElement);
      }
      this.tooltipElement = null;
    }
    document.removeEventListener("mousemove", this.handleMouseMove);
    this.activeTooltipIdx = null;
  };

  private handleIconChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onloadend = () => {
        this.setState((prevState) => ({
          modalData: {
            ...prevState.modalData,
            icon: reader.result as string,
          },
        }));
      };
      reader.readAsDataURL(file);
    } else {
      this.setState((prevState) => ({
        modalData: {
          ...prevState.modalData,
          icon: null,
        },
      }));
    }
  };

  handleModalClose = () => {
    this.setState({ isModalClosing: true });
    setTimeout(() => {
      this.setState({
        modalData: {
          idx: null,
          icon: null,
          command: "",
          args: "",
        },
        isModalOpen: false,
        isModalClosing: false,
      });
    }, 300);
  };

  handleModalSave = () => {
    this.state.grid[this.state.modalData.idx!] = {
      icon: this.state.modalData.icon,
      command: this.state.modalData.command,
      args: this.state.modalData.args.split(" ").map((arg) => arg.trim()),
    };

    this.handleModalClose();
  };

  generateGrid = () => {
    const { inputRows, inputCols } = this.state;
    const newGrid = Array.from({ length: inputRows * inputCols }, () => ({
      icon: null,
      command: null,
      args: null,
    }));
    this.setState({ rows: inputRows, cols: inputCols, grid: newGrid });
  };

  generateConfig = () => {
    const buttons: { [path: string]: Button } = {};

    for (const [index, cell] of this.state.grid.entries()) {
      if (cell.icon || cell.command || (cell.args && cell.args.length > 0)) {
        buttons[`/default/${index}`] = {
          command: cell.command,
          args: cell.args,
          icon: cell.icon ? cell.icon.split(",")[1] : null,
        };
      }
    }

    const config = {
      buttons,
    };

    const blob = new Blob([JSON.stringify(config, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "config.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  render() {
    const {
      rows,
      cols,
      inputRows,
      inputCols,
      grid,
      modalData,
      isModalOpen,
      isModalClosing,
    } = this.state;

    return (
      <div className="App">
        <div className="controls">
          <label>
            Rows:
            <input
              type="number"
              value={inputRows}
              onChange={(e) =>
                this.setState({ inputRows: Number(e.target.value) })
              }
            />
          </label>
          <label>
            Columns:
            <input
              type="number"
              value={inputCols}
              onChange={(e) =>
                this.setState({ inputCols: Number(e.target.value) })
              }
            />
          </label>
          <button onClick={this.generateGrid}>Generate Grid</button>
        </div>

        <div
          className="grid-container"
          style={{
            gridTemplateRows: `repeat(${rows}, 1fr)`,
            gridTemplateColumns: `repeat(${cols}, 1fr)`,
          }}
        >
          {grid.map((cell, index) => (
            <div
              key={index}
              className="grid-cell"
              onClick={() => this.handleCellClick(index)}
              onMouseEnter={(e) => this.handleCellMouseEnter(index, e)}
              onMouseLeave={() => this.handleCellMouseLeave()}
            >
              {cell.icon ? (
                <img src={cell.icon} alt="Icon" className="grid-cell-icon" />
              ) : (
                <span className="grid-cell-no-icon">No Icon</span>
              )}
            </div>
          ))}
        </div>

        {(isModalOpen || isModalClosing) && (
          <>
            <div
              className={`modal-overlay ${isModalClosing ? "fade-out" : ""}`}
              onClick={this.handleModalClose}
            ></div>
            <div className={`modal ${isModalClosing ? "fade-out" : ""}`}>
              <h2>Configure Button</h2>
              <label>
                Icon:
                <input
                  type="file"
                  accept="image/*"
                  onChange={this.handleIconChange}
                />
              </label>
              {modalData.icon && (
                <div className="modal-icon-preview-container">
                  <img
                    src={modalData.icon}
                    alt="Icon Preview"
                    className="modal-icon-preview-image"
                  />
                </div>
              )}
              <label>
                Command:
                <input
                  type="text"
                  value={modalData.command}
                  onChange={(e) =>
                    this.setState({
                      modalData: { ...modalData, command: e.target.value },
                    })
                  }
                />
              </label>
              <label>
                Args:
                <input
                  type="text"
                  value={modalData.args}
                  onChange={(e) =>
                    this.setState({
                      modalData: { ...modalData, args: e.target.value },
                    })
                  }
                />
              </label>
              <div className="modal-buttons">
                <button onClick={this.handleModalSave}>Save</button>
                <button onClick={this.handleModalClose}>Cancel</button>
              </div>
            </div>
          </>
        )}

        <button
          className="generate-config-button"
          onClick={this.generateConfig}
        >
          Generate Config
        </button>
      </div>
    );
  }
}

export default App;
