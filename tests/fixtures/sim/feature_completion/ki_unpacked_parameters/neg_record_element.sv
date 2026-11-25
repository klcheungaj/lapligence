module tb;
  typedef struct { int x; string s; } rec;
  parameter rec RR [2] = '{'{1, "a"}, '{2, "b"}};
  rec r;
  int i;
  initial r = RR[i];
endmodule
