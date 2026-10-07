// SIM-013 A01: event controls and a level wait on nested values. A change of
// the observed member, element or size wakes its reader; a sibling member, an
// unselected element and an unchanged value do not; a changed selector moves
// the observation to the newly selected element (SV 9.4.2, 9.4.3, 7.2, 7.4).
`timescale 1ns / 1ns
module tb;
  typedef struct {
    int a;
    int b;
  } in_t;
  typedef struct {
    in_t in;
    int  k;
  } out_t;
  in_t s;
  out_t o;
  int arr[4] = '{10, 20, 20, 40};
  int i = 0;
  int d[];
  int q[$];
  int aa[string];
  int k = 1;
  string l_sa = "", l_oib = "", l_arr = "", l_c3 = "", l_d = "", l_dsz = "", l_q = "", l_aa = "";
  int t_wait = -1;

  initial begin
    d = new[3];
    q = '{1, 2, 3};
    aa["a"] = 5;
    aa["b"] = 6;
  end

  initial #0 forever begin @(s.a); l_sa = {l_sa, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(o.in.b); l_oib = {l_oib, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(arr[i]); l_arr = {l_arr, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(arr[3]); l_c3 = {l_c3, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(d[k]); l_d = {l_d, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(d.size()); l_dsz = {l_dsz, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(q[0]); l_q = {l_q, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(aa["b"]); l_aa = {l_aa, $sformatf(" %0t", $time)}; end
  initial begin
    #0 wait (o.in.a == 2 && arr[i] == 5);
    t_wait = $time;
  end

  initial begin
    #1 s.b = 5;
    #1 s.a = 3;
    #1 s.a = 3;
    #1 o.in.a = 1;
    #1 o.k = 2;
    #1 o.in.b = 9;
    #1 o = '{in: '{a: 1, b: 9}, k: 5};
    #1 o = '{in: '{a: 1, b: 10}, k: 5};
    #1 arr[1] = 21;
    #1 i = 1;
    #1 arr[0] = 11;
    #1 arr[1] = 22;
    #1 begin
      arr[2] = 22;
      i = 2;
    end
    #1 arr[1] = 0;
    #1 arr[2] = 5;
    #1 arr[3] = 41;
    #1 d[0] = 4;
    #1 d[1] = 5;
    #1 k = 2;
    #1 d = new[5] (d);
    #1 q.push_back(9);
    #1 q.push_front(0);
    #1 aa["a"] = 7;
    #1 aa["b"] = 8;
    #1 d[2] = 1;
    #1 o.in.a = 2;
    #1;
    $display("s.a:%s last=%0d", l_sa, s.a);
    $display("o.in.b:%s last=%0d", l_oib, o.in.b);
    $display("arr[i]:%s last=%0d", l_arr, arr[i]);
    $display("arr[3]:%s last=%0d", l_c3, arr[3]);
    $display("d[k]:%s last=%0d", l_d, d[k]);
    $display("d.size:%s last=%0d", l_dsz, d.size());
    $display("q[0]:%s last=%0d", l_q, q[0]);
    $display("aa[b]:%s last=%0d", l_aa, aa["b"]);
    $display("wait: %0d", t_wait);
    $finish(0);
  end
endmodule
