// SIM-019: array reductions (SV 7.12.3). Without `with` the result has the
// element type; a `with` expression sets the type (string length, record
// member, class property, converted real). An empty sum is zero.
module tb;
  typedef struct {
    string tag;
    int weight;
  } part_t;

  class Cell;
    shortint v;
    function new(shortint value);
      v = value;
    endfunction
  endclass

  byte b[$] = '{8'sd100, 8'sd100, 8'sd100};
  bit [3:0] nib[] = '{4'hc, 4'h6, 4'h3};
  string s[$] = '{"ab", "cde", "", "f"};
  real r[$] = '{1.5, 2.25, -0.75};
  part_t parts[$];
  Cell cells[$];
  Cell c;
  int empty[$];
  string se[$];
  int ia[string];
  byte narrow;

  initial begin
    $display("byte-wrap %0d", b.sum());
    $display("byte-wide %0d", b.sum() with (int'(item)));
    $display("nibble %h %h %h %h", nib.and(), nib.or(), nib.xor(), nib.product());
    $display("strings %0d %0d", s.sum() with (item.len()), s.product() with (item.len() + 1));
    $display("real %0d %0d", r.sum() with (int'(item * 4.0)), r.product() with (int'(item * 4.0)));

    parts.push_back('{"a", 7});
    parts.push_back('{"b", -2});
    parts.push_back('{"c", 10});
    $display("records %0d %0d", parts.sum() with (item.weight),
             parts.xor() with (item.weight));

    for (int i = 1; i <= 4; i++) begin
      c = new(shortint'(i * 1000));
      cells.push_back(c);
    end
    $display("class %0d", cells.sum() with (int'(item.v)));
    narrow = cells.sum() with (8'(item.v));
    $display("class-narrow %0d", narrow);

    ia["x"] = 3;
    ia["y"] = 4;
    $display("assoc %0d %0d", ia.sum() with (item * item), ia.product());

    $display("empty %0d %0d", empty.sum(), se.sum() with (item.len()));
    $finish(0);
  end
endmodule
