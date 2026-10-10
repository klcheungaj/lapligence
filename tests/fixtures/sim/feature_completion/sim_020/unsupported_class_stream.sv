// SIM-020 llg limit: streaming a class object's members is rejected.
module tb;
  class c_t;
    int x;
  endclass
  c_t c;
  logic [31:0] v;
  initial begin
    c = new;
    c.x = 5;
    v = {>>{c}};
    $finish;
  end
endmodule
