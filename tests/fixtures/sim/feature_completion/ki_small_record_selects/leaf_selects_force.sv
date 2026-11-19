// `force` and `release` of a constant element of a member array of a record
// of integral members held as one signal per leaf (`r.a[1]`): the element is
// a singular variable, so the force overrides later assignments and the
// released element keeps its value until the next one.
typedef struct {
  logic [7:0] t;
  int a[3];
} r_t;

module tb;
  r_t r;

  initial begin
    r.t = 8'h55;
    r.a[0] = 1;
    r.a[1] = 7;
    r.a[2] = 3;
    force r.a[1] = 9;
    #1 $display("f %h %0d %0d %0d", r.t, r.a[0], r.a[1], r.a[2]);
    r.a[1] = 3;
    r.a[0] = 5;
    #1 $display("o %0d %0d", r.a[0], r.a[1]);
    release r.a[1];
    #1 $display("r %0d", r.a[1]);
    r.a[1] = 4;
    #1 $display("a %0d", r.a[1]);
    $finish(0);
  end
endmodule
