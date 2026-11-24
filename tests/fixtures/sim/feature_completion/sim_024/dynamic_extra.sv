// Arguments that a run-time format does not consume print in their default
// formats, as after a literal format (IEEE 1800-2009 21.3.3); the first such
// case of a run warns once. $swrite interprets no run-time format.
module tb;
  typedef struct {int x; string y;} rec_t;
  typedef enum {OFF, ON} sw_t;
  string f, so, st = "str";
  int a = 5;
  rec_t us = '{3, "yy"};
  int q[$] = '{1, 2};
  sw_t sw = ON;
  reg [47:0] v = "ok";
  initial begin
    f = "x=%0d";
    $display("1:%s", $sformatf(f, a, st, v, sw, "lit"));
    $display("2:%s", $sformatf("x=%0d", a, st, v, sw, "lit"));
    $sformat(so, f, a, us, q);
    $display("3:%s", so);
    f = "%s|%p";
    $display("4:%s", $sformatf(f, st, us, "a%0db", a));
    $display("5:%s", $sformatf("%s|%p", st, us, "a%0db", a));
    f = "x=%0d";
    $swrite(so, f, a);
    $display("6:%s", so);
    // The format of the first call is not retained for the next one.
    $display("7:%s", $sformatf("%0d", $sformatf(f, a) == "x=5"));
    f = "%0d";
    $display("8:%s", $sformatf({f, "!"}, a, $sformatf(f, 1, 2)));
    $finish;
  end
endmodule
