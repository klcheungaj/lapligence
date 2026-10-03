// SV2009 7.4.6, 11.4.5, 6.24: fixed aggregate equality over storage,
// formal, return, cast, net and invalid-index operands.
typedef struct { logic [3:0] a; bit [1:0] b; } rec_t;
typedef rec_t rows_t [0:2];
typedef struct { rows_t rs; logic [7:0] c; } deep_t;
typedef logic [3:0] mat_t [0:1][0:1];
typedef struct { logic [3:0] a; logic [1:0] b; } net_rec_t;
typedef logic [5:0] bits6_t;
typedef logic [15:0] bits16_t;
typedef logic [3:0] quad_t [0:3];

module peer(input rec_t p, input quad_t q, output logic same_p, output logic same_q);
  rec_t local_rec;
  quad_t local_quad;
  initial begin
    local_rec = '{4'h3, 2'd1};
    local_quad = '{4'h1, 4'h2, 4'h3, 4'h4};
  end
  assign same_p = (p == local_rec);
  assign same_q = (q === local_quad);
endmodule

module tb;
  rec_t r1, r2, port_rec;
  deep_t d1, d2;
  mat_t m1, m2;
  rows_t ra, rb;
  rec_t grid [0:1][0:2];
  quad_t port_quad;
  net_rec_t nv;
  wire net_rec_t nw;
  logic same_p, same_q, res;
  logic [1:0] xi;
  int i, j;

  assign nw = nv;
  peer u_peer(.p(port_rec), .q(port_quad), .same_p(same_p), .same_q(same_q));

  function automatic rec_t mk(input logic [3:0] a, input bit [1:0] b);
    rec_t r;
    r.a = a;
    r.b = b;
    return r;
  endfunction
  function automatic logic eq_in(input rec_t x, input rec_t y);
    return x == y;
  endfunction
  function automatic logic eq_const_ref(const ref rows_t x, input rows_t y);
    return x == y;
  endfunction
  function automatic logic eq_inout(inout rec_t x, input rec_t y);
    x.b = 2'd3;
    return x == y;
  endfunction
  function automatic void copy_out(output rec_t x, input rec_t y);
    x = y;
  endfunction
  function automatic logic ne_mat(input mat_t x, input mat_t y);
    return x != y;
  endfunction
  function automatic mat_t mkm(input logic [3:0] v);
    mat_t m;
    m[0][0] = v; m[0][1] = v; m[1][0] = v; m[1][1] = v;
    return m;
  endfunction
  function automatic deep_t mkd(input logic [7:0] c);
    deep_t d;
    d.rs[0] = mk(4'h1, 2'd1);
    d.rs[1] = mk(4'h2, 2'd2);
    d.rs[2] = mk(4'h4, 2'd0);
    d.c = c;
    return d;
  endfunction
  task automatic check_ref(ref rec_t x, input rec_t y, output logic result);
    result = (x == y) && (x !== mk(4'h0, 2'd0));
  endtask

  initial begin
    r1 = '{4'b10x1, 2'd1};
    r2 = '{4'b10x1, 2'd1};
    $display("A %b %b %b %b", r1 == r2, r1 === r2, r1 != r2, r1 !== r2);
    r2.b = 2'd2;
    $display("B %b %b %b", r1 == r2, r1 === r2, r1 != r2);
    $display("C %b %b %b", r1 == mk(4'b10x1, 2'd1), mk(4'h9, 2'd1) == mk(4'h9, 2'd1),
             eq_in(r1, mk(4'b1001, 2'd1)));
    d1 = mkd(8'h5a);
    d2 = mkd(8'h5a);
    $display("D %b %b %b", d1 == d2, d1 == mkd(8'h5a), mkd(8'ha5) != d2);
    d2.rs[1].a = 4'bz010;
    $display("E %b %b %b %b", d1 == d2, d1 === d2, d1.rs == d2.rs, d1.rs[0] == d2.rs[0]);
    d2.rs[2].b = 2'd1;
    $display("F %b %b", d1 == d2, d1.rs[1:2] == d2.rs[1:2]);
    m1 = mkm(4'h3);
    m2 = mkm(4'h3);
    $display("G %b %b %b", m1 == m2, m1[0] == m2[1], ne_mat(m1, m2));
    m2[1][0] = 4'bxxxx;
    $display("H %b %b %b %b", m1 == m2, m1 === m2, m1 == mkm(4'h3), ne_mat(m1, m2));
    $display("I %b %b %b", rec_t'(r1) == r1, rec_t'(bits6_t'(6'b0011_01)) == mk(4'h3, 2'd1),
             rec_t'(6'b0011_x1) === mk(4'h3, 2'd1));
    $display("J %b %b", quad_t'(16'h1234) == quad_t'{4'h1, 4'h2, 4'h3, 4'h4},
             bits16_t'(mkm(4'hc)) == 16'hcccc);
    foreach (ra[k]) begin
      ra[k] = mk(4'(k), 2'(k));
      rb[k] = ra[k];
    end
    $display("K %b %b", eq_const_ref(ra, rb), eq_const_ref(ra, '{mk(4'h0, 2'd0), mk(4'h1, 2'd1), mk(4'h2, 2'd3)}));
    r2 = '{4'h5, 2'd2};
    res = eq_inout(r2, mk(4'h5, 2'd3));
    $display("L %b %b", res, r2.b);
    copy_out(r2, mk(4'h7, 2'd1));
    check_ref(r2, mk(4'h7, 2'd1), res);
    $display("M %b %b", r2 == mk(4'h7, 2'd1), res);
    foreach (grid[p, q]) grid[p][q] = mk(4'(p * 3 + q), 2'(q));
    i = 4;
    j = 1;
    xi = 2'bx1;
    $display("N %b %b %b %b", ra[i] == ra[2], ra[xi] == ra[2], ra[i] === rb[7], ra[i] === mk(4'bxxxx, 2'd0));
    $display("O %b %b %b", grid[i] == grid[j], grid[j][i] == grid[0][0], grid[j][i] === mk(4'bxxxx, 2'd0));
    nv = '{4'h3, 2'd1};
    port_rec = '{4'h3, 2'd1};
    port_quad = '{4'h1, 4'h2, 4'h3, 4'h4};
    #1 $display("P %b %b %b %b", nw == nv, nv == nw, same_p, same_q);
    nv.b = 2'bz1;
    port_rec.b = 2'd2;
    port_quad[3] = 4'bz100;
    #1 $display("Q %b %b %b %b", nw == nv, nw === nv, same_p, same_q);
    $finish(0);
  end
endmodule
