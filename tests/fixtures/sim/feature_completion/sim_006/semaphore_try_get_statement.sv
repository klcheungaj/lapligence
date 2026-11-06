module tb;
  semaphore sm;
  initial begin
    sm = new(2);
    void'(sm.try_get(1));
    void'(sm.try_get(1));
    $display("%0d", sm.try_get(1));
    sm.put(1);
    $display("%0d", sm.try_get());
    $finish;
  end
endmodule
