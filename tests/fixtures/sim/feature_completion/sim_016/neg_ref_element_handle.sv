// SIM-016 boundary: a `ref` semaphore formal bound to an array element is
// legal (SV 13.5.2) but needs retained handle element references; rejected
// explicitly.
module tb;
  semaphore pool [2];
  task automatic use_ref(ref semaphore s);
    s.put(1);
  endtask
  initial begin
    pool[0] = new(0);
    use_ref(pool[0]);
  end
endmodule
