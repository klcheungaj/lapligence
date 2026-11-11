// SIM-040: open-array formals whose elements are real, shortreal, string or
// chandle values or structures holding them take fixed-size actuals of any
// range (SV 35.5.6.1); the handle addresses a C-layout copy (H.7.3, H.12.4)
// in every direction. Build native_open_arrays.c into a shared library and
// pass it with --dpi-lib.
module tb;
    typedef struct { real r; string s; int i; } rec_t;

    import "DPI-C" function chandle no_handle(input int id);
    import "DPI-C" function int no_id(input chandle h);
    import "DPI-C" function real no_sum(input string tag, input real a []);
    import "DPI-C" function void no_scale(inout real a [], input shortreal by []);
    import "DPI-C" function void no_grid(input string tag, inout real g [][]);
    import "DPI-C" function void no_rows(input string tag, input real g [][2]);
    import "DPI-C" function void no_strings(input string a [], output string o [],
        inout string io [][]);
    import "DPI-C" function void no_handles(input chandle a [], output chandle o []);
    import "DPI-C" function void no_records(input rec_t a [], inout rec_t io []);

    real two [2];
    real down [4:2];
    shortreal by [1:2];
    real grid [1:2][3];
    real wide [3][2];
    string names [3];
    string out2 [5:4];
    string table2 [2][2];
    chandle ha [2];
    chandle ho [1:0];
    rec_t ra [1:2];
    rec_t rio [2];

    initial begin
        two[0] = 1.25; two[1] = 2.5;
        down[4] = 4.0; down[3] = 3.0; down[2] = 2.0;
        $display("sum two=%g", no_sum("two", two));
        $display("sum down=%g", no_sum("down", down));

        by[1] = 2.0; by[2] = 0.5;
        no_scale(two, by);
        $display("two=%g %g", two[0], two[1]);

        foreach (grid[i, j]) grid[i][j] = i * 10 + j;
        no_grid("grid", grid);
        $display("grid=%g %g %g %g %g %g", grid[1][0], grid[1][1], grid[1][2], grid[2][0],
            grid[2][1], grid[2][2]);
        foreach (wide[i, j]) wide[i][j] = i + 0.5 * j;
        no_rows("wide", wide);

        names[0] = "n0"; names[1] = "n1"; names[2] = "n2";
        out2[5] = "old5"; out2[4] = "old4";
        table2[0][0] = "t00"; table2[0][1] = "t01"; table2[1][0] = "t10"; table2[1][1] = "t11";
        no_strings(names, out2, table2);
        $display("out2[5]=%s out2[4]=%s table2=%s %s %s %s", out2[5], out2[4], table2[0][0],
            table2[0][1], table2[1][0], table2[1][1]);

        ha[0] = no_handle(1); ha[1] = no_handle(2); ho[1] = no_handle(9);
        no_handles(ha, ho);
        $display("ho[1]=%0d ho[0]=%0d", no_id(ho[1]), no_id(ho[0]));

        ra[1].r = 1.5; ra[1].s = "one"; ra[1].i = 1;
        ra[2].r = 2.5; ra[2].s = "two"; ra[2].i = 2;
        rio[0].r = 10.0; rio[0].s = "a"; rio[0].i = 10;
        rio[1].r = 20.0; rio[1].s = "b"; rio[1].i = 20;
        no_records(ra, rio);
        $display("rio[0]=%g %s %0d rio[1]=%g %s %0d", rio[0].r, rio[0].s, rio[0].i, rio[1].r,
            rio[1].s, rio[1].i);
        $finish;
    end
endmodule
