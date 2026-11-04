// Bit and part selects of packed members of record elements.
module tb;
  typedef struct { logic [3:0] x; int y; } in_t;
  typedef struct {
    string s;
    int n;
    logic [7:0] v;
    logic [3:0][7:0] w;
    logic [8:15] up;
    logic [7:0] a[2];
    logic [8:15] b[2];
    logic [3:0][7:0] p[2];
    in_t in;
  } rec_t;
  rec_t nb[2];
  rec_t g[2][2];
  rec_t q[$];
  rec_t da[];
  rec_t aa[int];
  int k = 1, j = 9;
  logic b;
  initial begin
    // (A) Bit, part and indexed part selects, constant and run-time.
    nb[1].v = 8'h00;
    nb[1].v[2] = 1'b1;
    $display("%b", nb[1].v);
    nb[0].v = 8'hF0;
    nb[0].v[1:0] = 2'b11;
    $display("%b", nb[0].v);
    nb[k].v[k+:3] = 3'b101;
    $display("%b", nb[1].v);
    nb[k].v[k] = 1'b0;
    $display("%b", nb[1].v);
    nb[1].v[6-:2] = 2'b10;
    $display("%b %b %b", nb[1].v, nb[k].v[6:5], nb[1].v[k+1]);
    // (B) Two-state member and ascending range.
    nb[1].n = 0;
    nb[1].n[3] = 1'b1;
    nb[1].n[1:0] = 2'b01;
    $display("%0d", nb[1].n);
    nb[1].up = 8'h00;
    nb[1].up[8] = 1'b1;
    nb[1].up[j] = 1'b1;
    $display("%b %b", nb[1].up, nb[1].up[8:9]);
    // (C) Multidimensional packed member.
    nb[1].w = 0;
    nb[1].w[2] = 8'hAB;
    nb[1].w[k][3:0] = 4'hC;
    nb[1].w[k][7] = 1'b1;
    nb[1].w[3][k] = 1'b1;
    $display("%h %h %b", nb[1].w, nb[1].w[k], nb[1].w[2][1]);
    // (D) Elements of member arrays, sub-record members.
    nb[1].a[1] = 8'h03;
    nb[1].a[1][2] = 1'b1;
    nb[1].a[1][7:6] = 2'b11;
    nb[1].a[1][k+:2] = 2'b00;
    $display("%b %b %b", nb[1].a[1], nb[1].a[1][2], nb[1].a[1][7:6]);
    nb[0].b[1] = 8'h00;
    nb[0].b[1][8] = 1'b1;
    nb[0].b[1][j] = 1'b1;
    $display("%b %b %b", nb[0].b[1], nb[0].b[1][8], nb[0].b[1][10]);
    nb[1].p[0] = 0;
    nb[1].p[0][1] = 8'hAB;
    nb[1].p[0][2][3] = 1'b1;
    nb[1].p[0][j-8][0] = 1'b0;
    $display("%h %h %b", nb[1].p[0], nb[1].p[0][1], nb[1].p[0][2][3]);
    nb[1].in.x = 0;
    nb[1].in.x[1] = 1'b1;
    nb[k].in.y = 0;
    nb[k].in.y[k+:2] = 2'b11;
    $display("%b %0d", nb[1].in.x, nb[1].in.y);
    // (E) Compound writes.
    nb[1].w = 0;
    nb[1].w[1][3] |= 1'b1;
    nb[1].w[1] += 8'h01;
    nb[k].w[k] += 8'h01;
    nb[k].v[3:2] = 2'b01;
    nb[k].v[3:2] += 2'b01;
    nb[k].v[k] ^= 1'b1;
    $display("%h %b", nb[1].w, nb[1].v);
    // (F) Two-dimensional arrays, queues, dynamic and associative arrays.
    g[1][0].v = 8'h0;
    g[1][k].v[k] = 1'b1;
    $display("%b %b", g[1][0].v, g[1][1].v);
    q.push_back(nb[0]);
    q[0].v = 8'h00;
    q[0].v[2] = 1'b1;
    q[k-1].v[5:4] = 2'b11;
    q[0].in.x = 4'h0;
    q[0].in.x[2] = 1'b1;
    q[0].a[0] = 8'h00;
    q[0].a[0][7] = 1'b1;
    $display("%b %b %b", q[0].v, q[0].in.x, q[0].a[0]);
    da = new[1];
    da[0].n = 0;
    da[0].n[k] = 1'b1;
    $display("%0d", da[0].n);
    aa[3].v = 8'h00;
    aa[3].v[1] = 1'b1;
    $display("%b", aa[3].v);
    // (G) Selected bits read.
    b = nb[1].a[1][7];
    $display("%b", b);
    $finish(0);
  end
endmodule
