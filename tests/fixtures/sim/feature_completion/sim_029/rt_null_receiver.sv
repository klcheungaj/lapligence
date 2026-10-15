// S29-D9: randomize() through a null handle is a run-time error at the call.
class c_c; rand bit [3:0] a; endclass
module tb;
  int ok;
  initial begin
    static c_c c;
    $display("before");
    ok = c.randomize();
    $display("after %0d", ok);
    $finish;
  end
endmodule
