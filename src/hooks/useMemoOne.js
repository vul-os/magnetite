import { useRef } from 'react';

function depsMatch(prevDeps, nextDeps) {
  if (!prevDeps || !nextDeps) return false;
  if (prevDeps.length !== nextDeps.length) return false;
  return prevDeps.every((dep, i) => Object.is(dep, nextDeps[i]));
}

export function useMemoOne(fn, deps) {
  // This is a deliberate userland memoization primitive: it intentionally reads
  // and writes a ref during render to cache a value across renders with a single,
  // stable identity. Reading/writing the ref here is the entire point of the hook,
  // so the react-hooks/refs guard does not apply.
  /* eslint-disable react-hooks/refs */
  const ref = useRef({ deps, result: null, computed: false });
  if (!ref.current.computed || !depsMatch(ref.current.deps, deps)) {
    ref.current.deps = deps;
    ref.current.result = fn();
    ref.current.computed = true;
  }
  return ref.current.result;
  /* eslint-enable react-hooks/refs */
}
