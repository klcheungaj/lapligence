// SIM-025 limitation: a member of an unpacked structure property publishes no
// change, so a `$monitor` of it could never fire. `$strobe` reads it.
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  class C;
    rec_t r;
  endclass
  C h;
  initial begin
    h = new;
    $strobe("a=%0d", h.r.a);
    $monitor("a=%0d", h.r.a);
    #1 h.r.a = 1;
    #1 $finish(0);
  end
endmodule
