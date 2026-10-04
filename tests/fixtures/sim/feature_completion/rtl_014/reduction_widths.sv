// IEEE 1800-2009 7.12.3: a reduction's width and sign come from the element
// or with map, not from the destination; the first element seeds the result,
// so a singleton keeps Z. Record maps and selected rows reduce in place.
module tb;
  typedef struct packed {
    logic [3:0] tag;
    logic signed [7:0] v;
  } rec_t;
  typedef struct {
    byte k;
    logic [3:0] w;
  } urec_t;

  bit [3:0] a [8];
  logic [3:0] z [1];
  logic [3:0] zz [2];
  byte signed sb [4];
  rec_t r [3];
  urec_t u [3];
  bit [5:0] rows [0:2][0:19];
  int dst;
  logic [31:0] w;
  integer i;

  initial begin
    foreach (a[j]) a[j] = 4'(j + 9);
    dst = a.sum();
    $display("sum4=%0d", dst);
    dst = a.sum() with (int'(item));
    $display("sum32=%0d", dst);
    dst = a.product();
    $display("product4=%0d", dst);
    w = a.xor();
    $display("xor=%h", w);
    $display("and=%b or=%b", a.and(), a.or());
    z[0] = 4'bz;
    w = z.sum();
    $display("singleton=%b", w[3:0]);
    $display("and=%b or=%b xor=%b product=%b", z.and(), z.or(), z.xor(), z.product());
    zz[0] = 4'bz;
    zz[1] = 4'd1;
    $display("pair=%b", zz.sum());
    sb = '{-100, -100, 50, 1};
    dst = sb.sum();
    $display("signed8=%0d", dst);
    dst = sb.sum() with (int'(item));
    $display("signed32=%0d", dst);
    foreach (r[j]) r[j] = '{tag: 4'(j), v: 8'(j * 40 - 50)};
    $display("record=%0d", r.sum() with (int'(item.v)));
    foreach (u[j]) u[j] = '{k: byte'(j * 3), w: 4'(15 - j)};
    $display("unpacked=%0d", u.sum() with (int'(item.k) * item.w));
    foreach (rows[j, l]) rows[j][l] = 6'(j * 20 + l);
    i = 1;
    $display("row6=%0d row32=%0d", rows[i].sum(), rows[i].sum() with (int'(item)));
    i = 'x;
    $display("unselected=%b", rows[i].sum());
    $finish(0);
  end
endmodule
