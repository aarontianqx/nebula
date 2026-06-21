export function SafetyCard() {
  return (
    <>
      <h3>Safety</h3>
      <div className="card safety-card">
        <div className="safety-info">
          <span className="safety-icon">!</span>
          <div>
            <div className="safety-title">Emergency Stop</div>
            <div className="safety-key">Ctrl + Shift + Backspace</div>
          </div>
        </div>
      </div>
    </>
  );
}
