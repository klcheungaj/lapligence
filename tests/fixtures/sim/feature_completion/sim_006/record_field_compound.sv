module tb;
  typedef struct { int a; int arr[3]; } rec_t;
  typedef struct { int a; int b; } p_t;
  rec_t q[$];
  p_t q2[$];
  rec_t fa[2];
  rec_t aa[int];
  rec_t aat[int];
  logic [3:0] w[$];
  initial begin
    q.push_back('{1, '{0, 1, 2}});
    q2.push_back('{1, 2});
    q2[0].b += 2;
    fa[1].a += 2;
    fa[1].arr[2] -= 1;
    aa[3].arr[1] <<= 2;
    aa[3].a = 3;
    aa[3].a <<= 2;
    q[0].arr[2]++;
    w.push_back(4'h3);
    w[0] |= 4'h8;
    $display("%0d %0d %0d %0d %0d %0d %0h", q2[0].b, fa[1].a, fa[1].arr[2], aa[3].a, aa[3].arr[1], q[0].arr[2], w[0]);
  end
  initial begin
    #1 fa[0].a <= 3;
    fa[0].arr[1] <= 4;
    #1 $display("%0d %0d", fa[0].a, fa[0].arr[1]);
  end
  initial #3 $finish;
endmodule
