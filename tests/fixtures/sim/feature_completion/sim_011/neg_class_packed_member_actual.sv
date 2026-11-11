// SIM-011 boundary: a member of a packed element of a class array property is
// written by an assignment statement (`o.a[i].m = v`); as an output actual it
// rejects with its location.
typedef struct packed { logic [3:0] a; logic [3:0] m; } p_t;
class c;
  p_t a[3];
endclass
module tb;
  c o;
  int i = 1;
  task automatic setv(output logic [3:0] v); v = 4'h5; endtask
  initial begin
    o = new;
    setv(o.a[i].m);
    $display("%h", o.a[1]);
  end
endmodule
