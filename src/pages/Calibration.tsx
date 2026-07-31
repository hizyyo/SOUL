import { useState, useCallback } from 'react';
import {
  CALIBRATION_STEPS,
  TOTAL_STEPS,
  type CalibrationQuestion,
  type CalibrationAnswer,
} from '../data/calibration';

interface CalibrationProps {
  soulId: string;
  initialStep: number;
  initialAnswers: CalibrationAnswer[];
  onSave: (step: number, answers: CalibrationAnswer[]) => void;
  onComplete: (answers: CalibrationAnswer[]) => void;
  onBack: () => void;
}

export function Calibration({
  initialStep,
  initialAnswers,
  onSave,
  onComplete,
  onBack,
}: CalibrationProps) {
  const [stepIdx, setStepIdx] = useState(initialStep);
  const [answers, setAnswers] = useState<CalibrationAnswer[]>(initialAnswers);
  const [saving, setSaving] = useState(false);

  const getAnswer = useCallback(
    (qid: string) => {
      return answers.find((a) => a.questionId === qid);
    },
    [answers],
  );

  const setAnswer = useCallback((qid: string, value: string | string[]) => {
    setAnswers((prev) => {
      const idx = prev.findIndex((a) => a.questionId === qid);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = { questionId: qid, value };
        return next;
      }
      return [...prev, { questionId: qid, value }];
    });
  }, []);

  const currentStep = CALIBRATION_STEPS[stepIdx];
  if (!currentStep) {
    return null;
  }

  const handleNext = async () => {
    setSaving(true);
    try {
      await onSave(stepIdx + 1, answers);
      if (stepIdx < TOTAL_STEPS - 1) {
        setStepIdx(stepIdx + 1);
      } else {
        onComplete(answers);
      }
    } finally {
      setSaving(false);
    }
  };

  const handleBack = () => {
    if (stepIdx > 0) {
      setStepIdx(stepIdx - 1);
    } else {
      onBack();
    }
  };

  const allAnswered = currentStep.questions
    .filter((q) => !q.optional)
    .every((q) => {
      const a = getAnswer(q.id);
      if (!a) return false;
      if (Array.isArray(a.value)) return a.value.length > 0;
      return String(a.value).trim().length > 0;
    });

  return (
    <div>
      <div style={{ marginBottom: '16px' }}>
        <div style={{ fontSize: '12px', color: '#888', marginBottom: '4px' }}>
          Step {stepIdx + 1} of {TOTAL_STEPS} — {currentStep.title}
        </div>
        <div style={{ height: '4px', background: '#e5e7eb', borderRadius: '2px' }}>
          <div
            style={{
              height: '100%',
              width: `${((stepIdx + 1) / TOTAL_STEPS) * 100}%`,
              background: '#6366f1',
              borderRadius: '2px',
              transition: 'width 0.3s',
            }}
          />
        </div>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        {currentStep.questions.map((q) => (
          <QuestionRow
            key={q.id}
            question={q}
            answer={getAnswer(q.id)}
            onAnswer={(v) => setAnswer(q.id, v)}
          />
        ))}
      </div>

      <div
        style={{ marginTop: '20px', display: 'flex', gap: '8px', justifyContent: 'space-between' }}
      >
        <button onClick={handleBack} style={secondaryBtnStyle}>
          {stepIdx === 0 ? 'Back' : 'Previous'}
        </button>
        <button
          onClick={handleNext}
          disabled={!allAnswered || saving}
          style={{ ...primaryBtnStyle, opacity: allAnswered && !saving ? 1 : 0.5 }}
        >
          {saving ? 'Saving...' : stepIdx < TOTAL_STEPS - 1 ? 'Next' : 'Finish'}
        </button>
      </div>
    </div>
  );
}

function QuestionRow({
  question,
  answer,
  onAnswer,
}: {
  question: CalibrationQuestion;
  answer: CalibrationAnswer | undefined;
  onAnswer: (v: string | string[]) => void;
}) {
  const val = answer?.value ?? '';

  if (question.type === 'binary' && question.options) {
    return (
      <div style={{ padding: '12px', border: '1px solid #e5e7eb', borderRadius: '8px' }}>
        <p style={{ margin: '0 0 8px', fontWeight: 500 }}>{question.prompt}</p>
        <div style={{ display: 'flex', gap: '8px' }}>
          {question.options.map((opt) => (
            <button
              key={opt}
              onClick={() => onAnswer(opt)}
              style={{
                padding: '6px 16px',
                borderRadius: '6px',
                border: val === opt ? '2px solid #6366f1' : '1px solid #d1d5db',
                background: val === opt ? '#eef2ff' : '#fff',
                cursor: 'pointer',
                fontWeight: val === opt ? 600 : 400,
              }}
            >
              {opt}
            </button>
          ))}
        </div>
      </div>
    );
  }

  if (question.type === 'multiple' && question.options) {
    const selected = Array.isArray(val) ? val : val ? [val] : [];
    return (
      <div style={{ padding: '12px', border: '1px solid #e5e7eb', borderRadius: '8px' }}>
        <p style={{ margin: '0 0 8px', fontWeight: 500 }}>{question.prompt}</p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
          {question.options.map((opt) => {
            const isSel = selected.includes(opt);
            return (
              <label
                key={opt}
                style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}
              >
                <input
                  type="radio"
                  name={question.id}
                  checked={isSel}
                  onChange={() => onAnswer(opt)}
                />
                {opt}
              </label>
            );
          })}
        </div>
      </div>
    );
  }

  if (question.type === 'text' || question.type === 'writing') {
    return (
      <div style={{ padding: '12px', border: '1px solid #e5e7eb', borderRadius: '8px' }}>
        <p style={{ margin: '0 0 8px', fontWeight: 500 }}>
          {question.prompt}
          {question.optional && (
            <span style={{ color: '#888', fontWeight: 400, fontSize: '12px' }}> (optional)</span>
          )}
        </p>
        <textarea
          value={typeof val === 'string' ? val : ''}
          onChange={(e) => onAnswer(e.target.value)}
          style={{
            width: '100%',
            minHeight: '60px',
            padding: '8px',
            border: '1px solid #d1d5db',
            borderRadius: '6px',
            resize: 'vertical',
          }}
          placeholder="Type your answer..."
        />
      </div>
    );
  }

  return null;
}

const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 20px',
  background: '#6366f1',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontWeight: 600,
};

const secondaryBtnStyle: React.CSSProperties = {
  padding: '8px 20px',
  background: '#f3f4f6',
  color: '#333',
  border: '1px solid #d1d5db',
  borderRadius: '6px',
  cursor: 'pointer',
};
