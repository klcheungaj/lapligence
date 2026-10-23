// Decision S13-D1: an output or inout actual is written as an assignment to
// it made when the subroutine returns. The selectors of the actual (element
// and bit indices, part-select bases, associative keys, the handle before a
// selected class property) are evaluated then, so changes the subroutine
// made to them are seen. An inout reads its actual at the call, so such
// selectors are evaluated twice. A whole class property (with no member or
// select after it) is bound at the call: a callee that changes the handle
// still writes the original object.
//
// IEEE 1800-2009 13.5 (SystemVerilog-1800-2009.txt L18476): "output // copy
// value out at end". 13.5.2 (L19185-19187): "an inout argument is copied
// twice: once from the actual into the argument when the subroutine is
// called and once from the argument into the actual when the subroutine
// returns." The text does not say when the actual's selectors are
// evaluated; llg follows industry-standard behaviour.
module tb;
  int a[4];
  int q[$];
  int aa[string];
  logic [7:0] v;
  int i, n;
  string key;

  class C;
    int f;
    logic [7:0] bits;
    int arr[4];
    int idx;
    task automatic set(output int x, input int ni);
      idx = ni;
      x = 9;
    endtask
  endclass
  C c, d, h;
  C hs[3];

  function automatic int next_index();
    n++;
    return n;
  endfunction

  task automatic out_set_i(output int x, input int ni);
    i = ni;
    x = 77;
  endtask

  task automatic add_set_i(inout int x, input int ni);
    i = ni;
    x = x + 100;
  endtask

  function automatic int f_set_i(output int x, input int ni);
    i = ni;
    x = 66;
    return 1;
  endfunction

  task automatic bit_set_i(output logic x, input int ni);
    i = ni;
    x = 1'b1;
  endtask

  task automatic nibble_set_i(output logic [7:0] x, input int ni);
    i = ni;
    x = 8'hab;
  endtask

  task automatic set_key(output int x);
    key = "late";
    x = 5;
  endtask

  task automatic delayed(output int x);
    #1 x = 33;
  endtask

  task automatic swap_handle(output int x);
    h = d;
    x = 7;
  endtask

  task automatic handle_set_i(output C x, input int ni);
    i = ni;
    x = d;
  endtask

  task automatic swap_handle_bit(output logic x);
    h = d;
    x = 1'b1;
  endtask

  initial begin
    a = '{0, 0, 0, 0};
    i = 1;
    out_set_i(a[i], 3);
    $display("task output a[i]: %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);

    a = '{1, 2, 3, 4};
    i = 0;
    add_set_i(a[i], 2);
    $display("task inout a[i]: %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);

    a = '{1, 2, 3, 4};
    n = 0;
    add_set_i(a[next_index()], 0);
    $display("inout selector call: %0d %0d %0d %0d n=%0d", a[0], a[1], a[2], a[3], n);

    a = '{0, 0, 0, 0};
    i = 1;
    n = f_set_i(a[i], 3);
    $display("function output a[i]: %0d %0d %0d %0d", a[0], a[1], a[2], a[3]);

    v = 8'h00;
    i = 1;
    bit_set_i(v[i], 5);
    $display("bit select: %b", v);

    a = '{0, 0, 0, 0};
    v = 8'h00;
    i = 0;
    nibble_set_i({a[i][3:0], v[i+:4]}, 2);
    $display("concatenation: %0d %0d %0d %0d %b", a[0], a[1], a[2], a[3], v);

    aa.delete();
    key = "early";
    set_key(aa[key]);
    $display("associative key: early=%0d late=%0d", aa.exists("early"), aa["late"]);

    q = '{10, 20, 30};
    i = 0;
    fork
      delayed(q[i]);
    join_none
    #0 i = 1;
    q.push_front(9);
    #2 $display("queue after push_front: %0d %0d %0d %0d", q[0], q[1], q[2], q[3]);

    c = new;
    d = new;
    c.arr = '{0, 0, 0, 0};
    c.idx = 0;
    c.set(c.arr[c.idx], 3);
    $display("method output: %0d %0d %0d %0d", c.arr[0], c.arr[1], c.arr[2], c.arr[3]);

    h = c;
    swap_handle(h.f);
    $display("whole property: c.f=%0d d.f=%0d", c.f, d.f);

    h = c;
    c.bits = 8'h00;
    d.bits = 8'h00;
    swap_handle_bit(h.bits[2]);
    $display("selected property: c.bits=%b d.bits=%b", c.bits, d.bits);

    i = 0;
    handle_set_i(hs[i], 2);
    $display("handle element: %0d %0d %0d", hs[0] != null, hs[1] != null, hs[2] != null);
    $finish;
  end
endmodule
