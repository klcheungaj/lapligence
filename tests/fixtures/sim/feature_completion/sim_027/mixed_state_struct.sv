// SIM-027 A03: memories that are members of a structure mixing two-state and
// four-state members. A load keeps X/Z digits in the four-state member and
// reads them as 0 in the two-state one (IEEE 1800-2009 21.4.2); a dump of
// each member reads back unchanged, and the other members keep their values.
module tb;
  typedef struct {
    logic [7:0] four [0:3];
    bit [7:0] two [0:3];
    bit [3:0] tag;
    logic [3:0] mark;
  } mixed_t;
  mixed_t module_record;
  logic [7:0] back [0:3];

  task automatic show(string label, logic [7:0] a, b, c, d);
    $display("%s %h %h %h %h", label, a, b, c, d);
  endtask

  task automatic exercise(string label, inout mixed_t record);
    $readmemh("digits.mem", record.four);
    $readmemh("digits.mem", record.two);
    show({label, " four"}, record.four[0], record.four[1], record.four[2], record.four[3]);
    show({label, " two"}, record.two[0], record.two[1], record.two[2], record.two[3]);
    $display("%s tag %h mark %h", label, record.tag, record.mark);
    $writememh("four.mem", record.four);
    $readmemh("four.mem", back);
    show({label, " four dump"}, back[0], back[1], back[2], back[3]);
    $writememh("two.mem", record.two);
    $readmemh("two.mem", back);
    show({label, " two dump"}, back[0], back[1], back[2], back[3]);
  endtask

  task automatic automatic_record;
    mixed_t record;
    record.tag = 4'h5;
    record.mark = 4'hz;
    $readmemh("digits.mem", record.four);
    $readmemh("digits.mem", record.two);
    show("automatic four", record.four[0], record.four[1], record.four[2], record.four[3]);
    show("automatic two", record.two[0], record.two[1], record.two[2], record.two[3]);
    $display("automatic tag %h mark %h", record.tag, record.mark);
    $writememh("two.mem", record.two);
    $readmemh("two.mem", back);
    show("automatic two dump", back[0], back[1], back[2], back[3]);
  endtask

  initial begin
    int fd;
    fd = $fopen("digits.mem", "w");
    $fwrite(fd, "1x\nz2\n3_4\nxz\n");
    $fclose(fd);
    module_record.tag = 4'ha;
    module_record.mark = 4'hx;
    $readmemh("digits.mem", module_record.four);
    $readmemh("digits.mem", module_record.two);
    show("module four", module_record.four[0], module_record.four[1],
         module_record.four[2], module_record.four[3]);
    show("module two", module_record.two[0], module_record.two[1],
         module_record.two[2], module_record.two[3]);
    $display("module tag %h mark %h", module_record.tag, module_record.mark);
    $writememh("four.mem", module_record.four);
    $readmemh("four.mem", back);
    show("module four dump", back[0], back[1], back[2], back[3]);
    automatic_record();
    exercise("inout", module_record);
    $finish;
  end
endmodule
