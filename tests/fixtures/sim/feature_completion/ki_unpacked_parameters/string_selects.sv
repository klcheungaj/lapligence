// Strings of a parameter with several run-time indices, empty strings, and
// characters of its strings.
module m;
  typedef struct { int x; string s[2]; } rec;
  parameter string S2 [2][3] = '{'{"a", "", "ccc"}, '{"dd", "e", "ff"}};
  parameter string SD [1:0][2:1] = '{'{"p", "q"}, '{"r", "s"}};
  parameter string S3 [2][2][2] =
      '{'{'{"000", "001"}, '{"010", "011"}}, '{'{"100", "101"}, '{"110", "111"}}};
  parameter rec RR [2] = '{'{1, '{"one", "uno"}}, '{2, '{"two", "due"}}};
  parameter string SE [3] = '{"", "cd", "ef"};
  int i, j, k;
  byte b;
  string t;

  task all;
    for (i = 0; i < 2; i++)
      for (j = 0; j < 3; j++) begin
        t = S2[i][j];
        $display("%0d %0d [%s] %0d", i, j, S2[i][j], t.len());
      end
    for (i = 0; i < 2; i++)
      for (j = 1; j < 3; j++) $display("%s", SD[i][j]);
    for (i = 0; i < 2; i++)
      for (j = 0; j < 2; j++)
        for (k = 0; k < 2; k++) $display("%s %s", S3[i][j][k], S3[i][1][k]);
    for (i = 0; i < 2; i++)
      for (j = 0; j < 2; j++) $display("%0d %s", RR[i].x, RR[i].s[j]);
    for (i = 0; i < 3; i++) $display("[%s] %b", SE[i], SE[i] == "");
  endtask

  initial begin
    all;
    i = 1; j = 2; k = 0;
    t = {S2[i][j], S2[0][j], S2[i - 1][j - k]};
    $display("%s %b %b", t, S2[i][j] == "ff", S2[i][j] < S2[0][j]);
    $display("%s %s %s", S2[1][0][1], S2[i][j][k], S3[i][0][k][2]);
    b = SE[1][0];
    $display("%0d %0d %0d %0d", b, SE[i][1], SE[2][k], RR[i].s[1][2]);
    $display("%b %b %0d", SE[1][0] == "c", SE[i][k] > SE[i][1], SE[1][0] + 1);
    k = 5;
    $display("%0d %0d", SE[1][k], SE[0][0]);
  end
endmodule
module tb;
  m u ();
  m #(.SE('{"xyz", "", "w"})) v ();
endmodule
