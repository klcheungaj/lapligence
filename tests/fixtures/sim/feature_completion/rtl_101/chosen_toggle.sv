// Toggle coverage counts packed variables, so a chosen column record adds
// nothing, as its leaf form adds nothing: 36 is `plain` (4) plus `k` (32), and
// the two toggled entries are bits 1:0 of `plain`.
typedef struct {
  string s;
  logic [3:0] x;
  int a[16];
} r_t;

module tb;
  r_t r;
  logic [3:0] plain = 0;
  int k = 3;

  initial begin
    $coverage_control(`SV_COV_START, `SV_COV_TOGGLE, `SV_COV_HIER, $root);
    #1 r.x = 5;
    r.a[k] = 3;
    plain = 3;
    #1 r.x = 2;
    r.a[k] = 0;
    plain = 0;
    #1 $display("%0d %0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_HIER, $root),
                $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_HIER, $root));
    $finish(0);
  end
endmodule
