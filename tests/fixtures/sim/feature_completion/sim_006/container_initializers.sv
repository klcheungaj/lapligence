// SIM-006: a resizable container declaration accepts any whole-container
// source as its initializer (`new[]`, `new[](src)`, a copy, a concatenation),
// and static initializers run in the declaration initialization schedule,
// after the declarations they read (SV 6.21, 7.5.1, 10.5).
module tb;
    int n = 3;
    int d[] = new[n];
    int q1[$] = '{4, 5};
    int q2[$] = q1;
    int d2[] = '{7, 8};
    int e[] = new[4](d2);
    int c[$] = {q1, 6};
    int k = d.size() + q2.size();
    function automatic int f(int m);
        int t[] = new[m];
        int u[$] = {m, m, m};
        return t.size() + u[m];
    endfunction
    initial $display("%0d %0d %0d %0d %0d %0d %0d", d.size(), q2[1], e.size(), e[1], c[2], k, f(2));
endmodule
