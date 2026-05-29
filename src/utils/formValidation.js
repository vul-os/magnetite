export const validators = {
  required: (value) => !value ? 'This field is required' : null,

  email: (value) => {
    if (!value) return null;
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return emailRegex.test(value) ? null : 'Invalid email address';
  },

  minLength: (min) => (value) => {
    if (!value) return null;
    return value.length >= min ? null : `Must be at least ${min} characters`;
  },

  maxLength: (max) => (value) => {
    if (!value) return null;
    return value.length <= max ? null : `Must be at most ${max} characters`;
  },

  password: (value) => {
    if (!value) return null;
    if (value.length < 8) return 'Must be at least 8 characters';
    if (!/[A-Z]/.test(value)) return 'Must contain uppercase letter';
    if (!/[a-z]/.test(value)) return 'Must contain lowercase letter';
    if (!/[0-9]/.test(value)) return 'Must contain number';
    return null;
  },

  match: (otherValue) => (value) => {
    if (!value) return null;
    return value === otherValue ? null : 'Values do not match';
  },
};

export function validate(value, rules) {
  for (const rule of rules) {
    const error = typeof rule === 'function' ? rule(value) : validators[rule]?.(value);
    if (error) return error;
  }
  return null;
}
