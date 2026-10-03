// llg-test-fixture: tests/fixtures/sim/container_selects/nested_selects.sv
// Select writes of a packed element of a nested dynamic array (IEEE
// 1800-2009 §7.5, §11.5.1): both container indices name the element and only
// the further select addresses its bits.
module tb;
    logic [69:0] nd[][];
    logic [69:0] row[];
    int i;

    initial begin
        nd = new[2];
        row = new[3];
        foreach (row[k]) row[k] = '0;
        nd[1] = row;
        i = 1;
        nd[1][2][69:66] = 4'ha;
        nd[i][i][0] = 1'b1;
        nd[i][i + 1][i * 4 +: 8] = 8'bzzzz_zzzz;
        $display("e2=%h e1=%h", nd[1][2], nd[1][1]);
        $display("reads=%h %h %b", nd[1][1][69:64], nd[i][2][69 -: 4], nd[i][2][12:3]);
        $display("row=%h", row[2]);
        $finish;
    end
endmodule
