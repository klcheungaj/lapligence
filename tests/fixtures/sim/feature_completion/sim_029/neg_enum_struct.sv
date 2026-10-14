// A random structure with an enum member would need a value domain per
// member; it is rejected until SIM-030.
typedef enum bit [1:0] {A = 0, B = 2} e_t;
typedef struct packed { e_t kind; bit [3:0] v; } s_t;
class c_c; rand s_t s; endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
