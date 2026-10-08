// SIM-019 negative: min/max need relational order or a with expression (SV 7.12.1).
module tb;
  typedef struct {
    int a;
    string b;
  } t_t;
  t_t p[$];
  t_t r[$];
  initial begin
    p.push_back('{1, "x"});
    r = p.max();
  end
endmodule
