// A member of a packed element of a class array property as a write target
// (`o.a[i].m = v`): the element is read, the member's bits replaced and the
// element written back, with constant and run-time indices, compound and
// increment operators, a bit select below the member, and inside a method.
// An element index outside the array writes nothing. IEEE 1800-2009 7.2.1
// (L7696-7697): a packed structure subdivides a vector into subfields
// "accessed as members"; 7.4.6 (L8234-8238).
typedef struct packed { logic [3:0] a; logic [3:0] m; } p_t;
class c;
  p_t a[3];
  p_t b[1:2];
  function void bump(int k);
    a[k].m++;
  endfunction
endclass

module tb;
  c o;
  int i = 1, n = 5;
  initial begin
    o = new;
    o.a[0] = '0; o.a[1] = '0; o.a[2] = '0;
    o.a[i].m = 4'h5;
    $display("%h %h", o.a[1], o.a[i].m);
    o.a[i].a = 4'h9; o.a[i].m += 2; o.a[0].a[1] = 1'b1;
    $display("%h %h %h", o.a[0], o.a[1], o.a[2]);
    o.a[n].m = 4'h3; o.b[2].m = 4'h7;
    $display("%h %h %h %h", o.a[0], o.a[1], o.a[2], o.b[2]);
    o.bump(2); o.a[2].m++;
    $display("%h", o.a[2]);
    $finish;
  end
endmodule
