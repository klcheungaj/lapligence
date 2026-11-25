// %p and %0p of parameters with strings and reals, and of their parts, print
// what variables with the same values print.
module m;
  typedef enum logic [1:0] {RED, GREEN, BLUE} color;
  typedef struct packed {logic [3:0] hi; logic [3:0] lo;} pair;
  typedef struct {int x; string s;} rec;
  typedef struct {color c; pair p; real r; string s[2]; int a[2]; rec n; logic [7:0] u;} all;
  parameter string SA [3] = '{"ab", "", "ef"};
  parameter string SD [1:0][2:1] = '{'{"p", "q"}, '{"r", "s"}};
  parameter real RA [2] = '{1.5, -2.25};
  parameter rec RC = '{7, "seven"};
  parameter rec RR [2] = '{'{1, "a"}, '{-2, "b"}};
  parameter all AL = '{GREEN, '{4'h3, 4'hx}, 0.5, '{"u", "v"}, '{4, 5}, '{9, "n"}, 8'h1z};
  string vsa [3] = '{"ab", "", "ef"};
  string vsd [1:0][2:1] = '{'{"p", "q"}, '{"r", "s"}};
  real vra [2] = '{1.5, -2.25};
  rec vrr [2] = '{'{1, "a"}, '{-2, "b"}};
  all val = '{GREEN, '{4'h3, 4'hx}, 0.5, '{"u", "v"}, '{4, 5}, '{9, "n"}, 8'h1z};
  string t;

  task same(string param, string variable);
    if (param == variable) $display("same");
    else $display("%s differs from %s", param, variable);
  endtask

  initial begin
    $display("%p", SA);
    $display("%p", RA);
    $display("%p", RC);
    $display("%p", RR);
    $display("%p", RR[1]);
    $display("%p %p %p", AL.n, AL.s, AL.a);
    t = $sformatf("%p", RC);
    $display("%s", t);
    same($sformatf("%p %0p", SA, SA), $sformatf("%p %0p", vsa, vsa));
    same($sformatf("%p %0p", SD, SD), $sformatf("%p %0p", vsd, vsd));
    same($sformatf("%p %0p", RA, RA), $sformatf("%p %0p", vra, vra));
    same($sformatf("%p %0p", RR, RR), $sformatf("%p %0p", vrr, vrr));
    same($sformatf("%p %0p", RR[1], RR[0]), $sformatf("%p %0p", vrr[1], vrr[0]));
    same($sformatf("%p %0p", AL, AL), $sformatf("%p %0p", val, val));
  end
endmodule
module tb;
  m u ();
  m #(.RC('{8, "eight"})) v ();
endmodule
