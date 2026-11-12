// ADV-019 family 5, deeper dynamic objects: the VPI application deep.c walks
// fixed-size arrays of strings and handles with more than one dimension,
// nested queues and associative arrays of queues, arrays of unpacked
// structures, static class properties (variables and an event) and the same
// shapes as automatics of a task frame (IEEE 1800-2009 37.17, 37.25, 37.27,
// 37.29), then writes values the design prints. See readme.md for the
// expected output.
module tb;
  typedef struct {
    int a;
    string s;
    struct {
      byte b;
    } in;
  } rec_t;
  class C;
    static int si = 4;
    static string ss = "st";
    static int sa[2];
    static event se;
    int x;
  endclass
  class D extends C;
    static int ds = 9;
  endclass
  string s2[2][3];
  C h2[2][2];
  int qq[$][$];
  int aq[int][$];
  rec_t rq[$];
  rec_t rf[2];
  D p;
  int woke;
  task automatic t();
    string sm[2][2];
    int nq[$][$];
    rec_t rr[$];
    sm[1][0] = "sm10";
    nq.push_back('{5, 6});
    rr.push_back('{6, "r", '{7}});
    $deep(2);
    $display("sm[1][0]=%s nq[0][1]=%0d rr[0].a=%0d rr[0].s=%s rr[0].in.b=%0d",
             sm[1][0], nq[0][1], rr[0].a, rr[0].s, rr[0].in.b);
  endtask
  initial begin
    @(C::se) woke = 1;
  end
  initial begin
    #1;
    s2[1][2] = "s12";
    s2[0][1] = "s01";
    h2[1][0] = new;
    qq.push_back('{1, 2});
    qq.push_back('{3});
    aq[5].push_back(50);
    aq[5].push_back(51);
    aq[-1].push_back(10);
    rq.push_back('{1, "one", '{8'd2}});
    rq.push_back('{3, "three", '{8'd4}});
    rf[1].a = 11;
    p = new;
    C::sa[1] = 21;
    $deep(1);
    #1;
    $display("s2[0][2]=%s qq[1][0]=%0d aq[5][1]=%0d rq[1].a=%0d rq[0].s=%s rq[1].in.b=%0d",
             s2[0][2], qq[1][0], aq[5][1], rq[1].a, rq[0].s, rq[1].in.b);
    $display("si=%0d ss=%s sa[0]=%0d ds=%0d woke=%0d", C::si, C::ss, C::sa[0], D::ds, woke);
    t();
  end
endmodule
