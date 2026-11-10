// One element of an associative array as a solve...before side is not served
// (the whole array is).
class c_c;
  rand bit [7:0] aa [int];
  rand bit [7:0] x;
  constraint c { aa[3] < x; solve aa[3] before x; }
endclass
module tb;
  initial begin
    static c_c c = new;
    c.aa[3] = 0;
    void'(c.randomize());
    $finish;
  end
endmodule
