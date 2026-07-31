export interface CalibrationQuestion {
  id: string;
  type: 'binary' | 'multiple' | 'text' | 'writing';
  category: 'preference' | 'decision' | 'boundary' | 'goal' | 'fact';
  prompt: string;
  options?: string[];
  optional: boolean;
}

export interface CalibrationAnswer {
  questionId: string;
  value: string | string[];
}

const BINARY: CalibrationQuestion[] = [
  { id: 'pref_1', type: 'binary', category: 'preference', prompt: 'Do you prefer concise answers or detailed explanations?', options: ['Concise', 'Detailed'], optional: false },
  { id: 'pref_2', type: 'binary', category: 'preference', prompt: 'Do you prefer bullet points or paragraphs?', options: ['Bullet points', 'Paragraphs'], optional: false },
  { id: 'pref_3', type: 'binary', category: 'preference', prompt: 'Do you prefer formal or casual tone?', options: ['Formal', 'Casual'], optional: false },
  { id: 'pref_4', type: 'binary', category: 'preference', prompt: 'Do you prefer practical examples or theoretical explanations?', options: ['Practical', 'Theoretical'], optional: false },
  { id: 'pref_5', type: 'binary', category: 'preference', prompt: 'When exploring ideas, do you prefer breadth or depth?', options: ['Breadth', 'Depth'], optional: false },
  { id: 'pref_6', type: 'binary', category: 'preference', prompt: 'Do you prefer to plan ahead or adapt as you go?', options: ['Plan ahead', 'Adapt as I go'], optional: false },
  { id: 'pref_7', type: 'binary', category: 'preference', prompt: 'Do you prefer proven solutions or experimental approaches?', options: ['Proven', 'Experimental'], optional: false },
  { id: 'pref_8', type: 'binary', category: 'preference', prompt: 'Do you prefer working alone or collaborating?', options: ['Alone', 'Collaborating'], optional: false },
  { id: 'pref_9', type: 'binary', category: 'preference', prompt: 'Do you prioritize speed or quality?', options: ['Speed', 'Quality'], optional: false },
  { id: 'pref_10', type: 'binary', category: 'preference', prompt: 'When making decisions, do you rely more on data or intuition?', options: ['Data', 'Intuition'], optional: false },
  { id: 'pref_11', type: 'binary', category: 'preference', prompt: 'Do you prefer to minimize risk or maximize opportunity?', options: ['Minimize risk', 'Maximize opportunity'], optional: false },
  { id: 'pref_12', type: 'binary', category: 'preference', prompt: 'Do you prefer structured processes or flexible workflows?', options: ['Structured', 'Flexible'], optional: false },
];

const MULTIPLE_CHOICE: CalibrationQuestion[] = [
  { id: 'goal_1', type: 'multiple', category: 'goal', prompt: 'What is your primary current goal?', options: ['Build a product', 'Grow a business', 'Learn a skill', 'Solve a specific problem', 'Explore new ideas'], optional: false },
  { id: 'goal_2', type: 'multiple', category: 'goal', prompt: 'What is your biggest working constraint?', options: ['Time', 'Budget', 'Team size', 'Technical complexity', 'Market uncertainty'], optional: false },
  { id: 'bound_1', type: 'multiple', category: 'boundary', prompt: 'What topics do you never want AI to decide without you?', options: ['Financial decisions', 'Legal commitments', 'Health advice', 'Relationship advice', 'Reputation management'], optional: false },
  { id: 'bound_2', type: 'multiple', category: 'boundary', prompt: 'What information should never be shared with AI models?', options: ['Passwords and secrets', 'Personal addresses', 'Financial accounts', 'Private conversations', 'Nothing is off-limits'], optional: false },
  { id: 'dec_1', type: 'multiple', category: 'decision', prompt: 'How do you typically evaluate a new tool or framework?', options: ['Community size', 'Documentation quality', 'Production readiness', 'Learning curve', 'Personal interest'], optional: false },
];

const TEXT_QUESTIONS: CalibrationQuestion[] = [
  { id: 'text_1', type: 'text', category: 'fact', prompt: 'Describe your work or main project in a few sentences.', optional: false },
  { id: 'text_2', type: 'text', category: 'goal', prompt: 'What is one principle you try to follow in your work?', optional: true },
  { id: 'text_3', type: 'text', category: 'boundary', prompt: 'Is there a hard line you never cross? (optional)', optional: true },
  { id: 'text_4', type: 'text', category: 'fact', prompt: 'What tools do you use daily?', optional: true },
  { id: 'text_5', type: 'text', category: 'fact', prompt: 'What is your professional background?', optional: true },
];

const WRITING_SAMPLES: CalibrationQuestion[] = [
  { id: 'write_1', type: 'writing', category: 'preference', prompt: 'Paste a short piece of text you wrote (email, message, doc).', optional: true },
  { id: 'write_2', type: 'writing', category: 'preference', prompt: 'Paste another example of your writing (different context).', optional: true },
  { id: 'write_3', type: 'writing', category: 'preference', prompt: 'One more writing sample (optional).', optional: true },
];

export const CALIBRATION_STEPS: { title: string; questions: CalibrationQuestion[] }[] = [
  { title: 'Preferences', questions: BINARY },
  { title: 'Goals & Boundaries', questions: MULTIPLE_CHOICE },
  { title: 'About You', questions: TEXT_QUESTIONS },
  { title: 'Writing Style', questions: WRITING_SAMPLES },
];

export const TOTAL_STEPS = CALIBRATION_STEPS.length;

export function answerToEntity(
  question: CalibrationQuestion,
  answer: string | string[],
): { type: string; data: Record<string, unknown> } | null {
  if (question.type === 'writing' || question.type === 'text') {
    if (!answer || (typeof answer === 'string' && !answer.trim())) return null;
    return {
      type: question.category,
      data: {
        claim: answer,
        category: question.category,
        source: 'calibration',
      },
    };
  }

  if (Array.isArray(answer)) {
    return {
      type: question.category,
      data: {
        claim: question.prompt,
        value: answer,
        source: 'calibration',
      },
    };
  }

  return {
    type: question.category,
    data: {
      claim: question.prompt,
      value: answer,
      source: 'calibration',
    },
  };
}
