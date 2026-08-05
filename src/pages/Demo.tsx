import { DEMO_ENTITIES, DEMO_NOTICE, INVESTOR_DEMO_STEPS } from '../data/demo';

export function Demo({ onExit }: { onExit: () => void }) {
  return (
    <section aria-labelledby="demo-title" data-testid="demo-screen">
      <div className="demo-banner" role="status">
        {DEMO_NOTICE}
      </div>
      <h2 id="demo-title" style={{ margin: '0 0 4px' }}>
        55-секундная демонстрация
      </h2>
      <p style={{ color: '#4b5563', marginTop: 0 }}>
        Этот экран использует только встроенные примеры. Он не читает и не меняет ваши данные.
      </p>

      <div className="demo-grid">
        <div className="demo-card">
          <h3>Сценарий</h3>
          <ol className="demo-steps">
            {INVESTOR_DEMO_STEPS.map((step) => (
              <li key={step.at}>
                <strong>
                  {step.at} · {step.title}
                </strong>
                <span>{step.detail}</span>
              </li>
            ))}
          </ol>
        </div>
        <div className="demo-card">
          <h3>Пример контекста</h3>
          {DEMO_ENTITIES.map((entity) => (
            <article className="demo-entity" key={entity.claim}>
              <span>{entity.type}</span>
              <p>{entity.claim}</p>
              <small>{entity.status}</small>
            </article>
          ))}
          <p className="demo-safe-note">
            Имитированный Gateway в этом режиме не запускается: внешние действия отсутствуют.
          </p>
        </div>
      </div>
      <button onClick={onExit} className="secondary-button">
        Выйти из демо
      </button>
    </section>
  );
}
