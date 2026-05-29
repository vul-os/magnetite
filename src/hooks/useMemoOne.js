import { useRef } from 'react';

function depsMatch(prevDeps, nextDeps) {
  if (!prevDeps || !nextDeps) return false;
  if (prevDeps.length !== nextDeps.length) return false;
  return prevDeps.every((dep, i) => Object.is(dep, nextDeps[i]));
}

export function useMemoOne(fn, deps) {
  const ref = useRef({ deps, result: null, computed: false });
  if (!ref.current.computed || !depsMatch(ref.current.deps, deps)) {
    ref.current.deps = deps;
    ref.current.result = fn();
    ref.current.computed = true;
  }
  return ref.current.result;
}
