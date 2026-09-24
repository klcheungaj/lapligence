// SYN-038 hierarchy/call witness: per-instance methods and parent dispatch.
module child #(parameter integer OFFSET = 0);
  integer state = 0;
  task bump(input integer amount, output integer result);
    state = state + amount + OFFSET;
    result = state;
  endtask
  function integer read(input integer amount);
    read = state + amount + OFFSET;
  endfunction
  initial begin
    #1 tb.parent_bump(3);
  end
endmodule

module tb;
  integer state = 0;
  integer a;
  integer b;
  child #(1) c0();
  child #(10) c1();
  task parent_bump(input integer amount);
    state = state + amount;
  endtask
  initial begin
    c0.bump(1, a);
    c1.bump(2, b);
    #2 $display("hier=%0d,%0d,%0d,%0d,%0d", state, a, b,
                c0.read(3), c1.read(3));
    $finish(0);
  end
endmodule
