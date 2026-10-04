// Unsupported boundary, not a language rule: a named event in the same list as
// a process-evaluated helper whose sensitivity includes unpacked-array storage.
module tb;
  event ev;
  int mem [0:3];
  int seen = 0;
  function int f(input int i);
    seen++;
    return mem[i];
  endfunction
  initial begin
    @(ev or f(1));
    $finish(0);
  end
endmodule
