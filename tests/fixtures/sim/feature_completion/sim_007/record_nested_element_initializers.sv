// FIN-001 R2: a record declaration initialized from an element of a nested
// container (`rec_t z = nas["x"][1]`), with a string member (`rec_t`) and with
// integral members only (`prec_t`). Run with `-D N=3` and `-D N=16`: the
// records then take both storage forms. A static declaration in a procedural
// block initializes once before time 0 (SV 6.21), when `nas["x"]` does not
// exist yet, so it reads the default record; an automatic one initializes at
// each entry and reads the element.
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; int big[`N]; } rec_t;
typedef struct { int a; int big[`N]; } prec_t;
typedef rec_t rows_t[];
typedef prec_t prows_t[];
module tb;
  function automatic rows_t rows();
    rows_t r = new[2];
    r[0].a = 1;
    r[0].s = "one";
    r[1].a = 2;
    r[1].s = "two";
    r[1].big[1] = 3;
    return r;
  endfunction
  function automatic prows_t prows();
    prows_t r = new[2];
    r[0].a = 21;
    r[1].a = 22;
    r[1].big[1] = 4;
    return r;
  endfunction
  rec_t nas[string][];
  prec_t pas[string][];
  rows_t early[string] = '{"x": rows()};
  prows_t pearly[string] = '{"x": prows()};
  function automatic int sum_from(string key);
    automatic rec_t f = early[key][1];
    return f.a + f.big[1];
  endfunction
  initial begin
    nas["x"] = new[2];
    nas["x"][1].a = 11;
    nas["x"][1].s = "eleven";
    nas["x"][1].big[1] = 12;
    pas["x"] = new[2];
    pas["x"][1].a = 13;
    $display("direct %0d '%s' %0d", nas["x"][1].a, nas["x"][1].s, pas["x"][1].a);
    begin
      static rec_t z = nas["x"][1];
      static prec_t p = pas["x"][1];
      $display("static before time 0 %0d '%s' %0d %0d", z.a, z.s, z.big[1], p.a);
    end
    begin
      automatic rec_t z = nas["x"][1];
      automatic prec_t p = pas["x"][1];
      $display("automatic %0d '%s' %0d %0d", z.a, z.s, z.big[1], p.a);
    end
    begin
      static rec_t z = early["x"][1];
      static prec_t p = pearly["x"][1];
      $display("static from initialized %0d '%s' %0d %0d %0d", z.a, z.s, z.big[1], p.a,
               p.big[1]);
    end
    for (int i = 0; i < 2; i++) begin
      automatic rec_t z = early["x"][i];
      $display("loop %0d '%s'", z.a, z.s);
    end
    $display("function %0d", sum_from("x"));
    $finish;
  end
endmodule
