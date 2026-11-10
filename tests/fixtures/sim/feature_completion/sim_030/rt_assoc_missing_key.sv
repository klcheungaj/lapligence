// A constraint that selects a key the associative array does not hold is an
// index outside the array (S30-D11): the call fails, no entry is created.
class c_c;
  rand bit [7:0] aa [int];
  constraint c { aa[4] == 9; }
endclass
module tb;
  c_c c = new;
  int ok;
  initial begin
    c.aa[1] = 0;
    ok = c.randomize();
    $display("ok=%0d entries=%0d", ok, c.aa.num());
    c.aa[4] = 0;
    ok = c.randomize();
    $display("ok=%0d entries=%0d aa[4]=%0d", ok, c.aa.num(), c.aa[4]);
    $finish;
  end
endmodule
