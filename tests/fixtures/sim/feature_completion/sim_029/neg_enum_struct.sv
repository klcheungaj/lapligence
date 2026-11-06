// An untagged union with an enum member has no per-member value domain: the
// members share their bits.
typedef enum bit [1:0] {A = 0, B = 2} e_t;
typedef union packed { e_t kind; bit [1:0] v; } s_t;
class c_c; rand s_t s; endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
