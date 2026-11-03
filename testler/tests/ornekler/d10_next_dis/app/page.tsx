export default function Anasayfa() {
  async function ode() {
    await fetch('/api/odeme', { method: 'POST', body: JSON.stringify({ tutar: 100 }) });
  }
  return <button onClick={ode}>Öde</button>;
}
