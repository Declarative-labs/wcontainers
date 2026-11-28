# Plants of the naked run (linux-process `direct`, `entering::run`)

Each instruction line of the naked run deleted alone, in a copy of its own, against `cargo test -p
wcontainers-linux-process-direct` (the conformance suite and the earlier-handler test). Driver: one deletion per line
from `naked_asm!(` to the first operand, every line that is an instruction (labels are not plants). Measured at
0132e8f2c on Graviton4 (job nr-asm 20261127-211213, 4 copies of 3 jobs each, a test limit of 150 s).

"caught (limit)" is a copy whose build and tests passed the 150 s limit at a machine load of about 300; it is not yet
a verdict and is run again with no limit at a lower load.

| Line at 0132e8f2c | Instruction deleted | Verdict |
|---|---|---|
| 1177 | `sub sp, sp, #{kept}` | caught |
| 1178 | `stp x19, x20, [sp, #0]` | caught |
| 1179 | `stp x21, x22, [sp, #16]` | caught |
| 1180 | `stp x23, x24, [sp, #32]` | caught |
| 1181 | `stp x25, x26, [sp, #48]` | caught |
| 1182 | `stp x27, x28, [sp, #64]` | caught (limit) |
| 1183 | `stp x29, x30, [sp, #80]` | caught |
| 1184 | `stp d8, d9, [sp, #{kept_d} + 0]` | caught (limit) |
| 1185 | `stp d10, d11, [sp, #{kept_d} + 16]` | caught (limit) |
| 1186 | `stp d12, d13, [sp, #{kept_d} + 32]` | caught (limit) |
| 1187 | `stp d14, d15, [sp, #{kept_d} + 48]` | **missed** |
| 1188 | `mrs x9, fpcr` | caught (limit) |
| 1189 | `mrs x10, tpidr_el0` | caught |
| 1190 | `stp x9, x10, [sp, #{kept_system}]` | caught |
| 1191 | `adr x9, 2f` | caught (limit) |
| 1192 | `mov x10, sp` | caught |
| 1193 | `stp x9, x10, [x0, #{exits_go}]` | caught |
| 1196 | `ldr x11, [x0, #{signal_stack}]` | caught |
| 1197 | `adr x9, 0b` | caught |
| 1198 | `adr x10, 3f` | caught |
| 1199 | `stp x9, x10, [x11, #{top_entry}]` | caught |
| 1200 | `adr x9, 2f` | caught (limit) |
| 1201 | `adr x10, 5f` | caught |
| 1202 | `stp x9, x10, [x11, #{top_exit}]` | caught |
| 1203 | `adr x9, 6f` | caught |
| 1204 | `adr x10, 4f` | caught (limit) |
| 1205 | `stp x9, x10, [x11, #{top_labels}]` | caught |
| 1206 | `str x0, [x11, #{top_running}]` | caught |
| 1207 | `ldr x9, [x11, #{top_pending}]` | caught (limit) |
| 1208 | `cbnz x9, 6f` | caught (limit) |
| 1210 | `mov x28, x0` | caught |
| 1211 | `add x9, x28, #{registers}` | caught |
| 1212 | `ldp q0, q1, [x9, #{v} + 0]` | caught (limit) |
| 1213 | `ldp q2, q3, [x9, #{v} + 32]` | caught (limit) |
| 1214 | `ldp q4, q5, [x9, #{v} + 64]` | caught (limit) |
| 1215 | `ldp q6, q7, [x9, #{v} + 96]` | caught (limit) |
| 1216 | `ldp q8, q9, [x9, #{v} + 128]` | caught |
| 1217 | `ldp q10, q11, [x9, #{v} + 160]` | caught |
| 1218 | `ldp q12, q13, [x9, #{v} + 192]` | caught (limit) |
| 1219 | `ldp q14, q15, [x9, #{v} + 224]` | caught (limit) |
| 1220 | `ldp q16, q17, [x9, #{v} + 256]` | caught (limit) |
| 1221 | `ldp q18, q19, [x9, #{v} + 288]` | caught (limit) |
| 1222 | `ldp q20, q21, [x9, #{v} + 320]` | caught |
| 1223 | `ldp q22, q23, [x9, #{v} + 352]` | caught |
| 1224 | `ldp q24, q25, [x9, #{v} + 384]` | caught (limit) |
| 1225 | `ldp q26, q27, [x9, #{v} + 416]` | caught |
| 1226 | `ldp q28, q29, [x9, #{v} + 448]` | caught |
| 1227 | `ldp q30, q31, [x9, #{v} + 480]` | caught |
| 1228 | `ldr x10, [x9, #{fpsr}]` | caught |
| 1229 | `msr fpsr, x10` | caught (limit) |
| 1230 | `ldr x10, [x9, #{fpcr}]` | caught (limit) |
| 1231 | `msr fpcr, x10` | caught |
| 1232 | `ldr x10, [x9, #{tpidr}]` | caught |
| 1233 | `msr tpidr_el0, x10` | caught (limit) |
| 1234 | `ldr x10, [x9, #{nzcv}]` | caught (limit) |
| 1235 | `and x10, x10, #{nzcv_bits}` | caught |
| 1236 | `msr nzcv, x10` | caught |
| 1237 | `ldr x10, [x9, #{sp}]` | caught (limit) |
| 1238 | `mov sp, x10` | caught (limit) |
| 1239 | `ldr x23, [x9, #{pc}]` | caught |
| 1240 | `add x9, x9, #{x}` | caught |
| 1241 | `ldp x0, x1, [x9, #0]` | caught (limit) |
| 1242 | `ldp x2, x3, [x9, #16]` | caught |
| 1243 | `ldp x4, x5, [x9, #32]` | caught (limit) |
| 1244 | `ldp x6, x7, [x9, #48]` | caught (limit) |
| 1245 | `ldp x10, x11, [x9, #80]` | caught (limit) |
| 1246 | `ldp x12, x13, [x9, #96]` | caught (limit) |
| 1247 | `ldp x14, x15, [x9, #112]` | caught (limit) |
| 1248 | `ldp x16, x17, [x9, #128]` | caught (limit) |
| 1249 | `ldp x18, x19, [x9, #144]` | caught |
| 1250 | `ldp x20, x21, [x9, #160]` | caught |
| 1251 | `ldr x22, [x9, #176]` | caught |
| 1252 | `ldp x24, x25, [x9, #192]` | caught |
| 1253 | `ldp x26, x27, [x9, #208]` | caught |
| 1254 | `ldr x29, [x9, #232]` | caught (limit) |
| 1255 | `ldr x30, [x9, #240]` | caught |
| 1256 | `ldr x8, [x9, #64]` | caught |
| 1257 | `ldr x9, [x9, #72]` | caught |
| 1259 | `br x23` | caught |
| 1262 | `add x23, x28, #{registers} + {x}` | caught |
| 1263 | `stp x0, x1, [x23, #0]` | caught (limit) |
| 1264 | `stp x2, x3, [x23, #16]` | caught |
| 1265 | `stp x4, x5, [x23, #32]` | caught |
| 1266 | `stp x6, x7, [x23, #48]` | caught (limit) |
| 1267 | `stp x8, x9, [x23, #64]` | caught |
| 1268 | `stp x10, x11, [x23, #80]` | caught (limit) |
| 1269 | `stp x12, x13, [x23, #96]` | caught (limit) |
| 1270 | `stp x14, x15, [x23, #112]` | caught |
| 1271 | `stp x16, x17, [x23, #128]` | caught (limit) |
| 1272 | `stp x18, x19, [x23, #144]` | caught (limit) |
| 1273 | `stp x20, x21, [x23, #160]` | caught (limit) |
| 1274 | `str x22, [x23, #176]` | caught |
| 1275 | `stp x24, x25, [x23, #192]` | caught (limit) |
| 1276 | `stp x26, x27, [x23, #208]` | caught |
| 1277 | `stp x28, x29, [x23, #224]` | caught |
| 1278 | `ldp x0, x1, [x28, #{exit}]` | caught |
| 1279 | `str x0, [x23, #184]` | caught (limit) |
| 1280 | `str x1, [x23, #240]` | caught |
| 1281 | `add x23, x28, #{registers}` | caught |
| 1282 | `str x1, [x23, #{pc}]` | caught |
| 1283 | `mov x0, sp` | caught |
| 1284 | `str x0, [x23, #{sp}]` | caught |
| 1285 | `mrs x0, nzcv` | caught |
| 1286 | `str x0, [x23, #{nzcv}]` | caught (limit) |
| 1287 | `sub x0, x30, #{exit_linked}` | caught |
| 1288 | `stp xzr, x0, [x28, #{end}]` | caught |
| 1291 | `add x23, x28, #{registers}` | caught |
| 1292 | `mrs x0, fpsr` | caught |
| 1293 | `str x0, [x23, #{fpsr}]` | **missed** |
| 1294 | `mrs x0, fpcr` | caught |
| 1295 | `str x0, [x23, #{fpcr}]` | caught (limit) |
| 1296 | `mrs x0, tpidr_el0` | caught (limit) |
| 1297 | `str x0, [x23, #{tpidr}]` | caught |
| 1298 | `stp q0, q1, [x23, #{v} + 0]` | caught (limit) |
| 1299 | `stp q2, q3, [x23, #{v} + 32]` | caught |
| 1300 | `stp q4, q5, [x23, #{v} + 64]` | caught |
| 1301 | `stp q6, q7, [x23, #{v} + 96]` | caught |
| 1302 | `stp q8, q9, [x23, #{v} + 128]` | caught (limit) |
| 1303 | `stp q10, q11, [x23, #{v} + 160]` | caught |
| 1304 | `stp q12, q13, [x23, #{v} + 192]` | caught |
| 1305 | `stp q14, q15, [x23, #{v} + 224]` | caught |
| 1306 | `stp q16, q17, [x23, #{v} + 256]` | caught (limit) |
| 1307 | `stp q18, q19, [x23, #{v} + 288]` | caught |
| 1308 | `stp q20, q21, [x23, #{v} + 320]` | caught (limit) |
| 1309 | `stp q22, q23, [x23, #{v} + 352]` | caught |
| 1310 | `stp q24, q25, [x23, #{v} + 384]` | caught (limit) |
| 1311 | `stp q26, q27, [x23, #{v} + 416]` | caught |
| 1312 | `stp q28, q29, [x23, #{v} + 448]` | caught (limit) |
| 1313 | `stp q30, q31, [x23, #{v} + 480]` | caught (limit) |
| 1316 | `ldr x11, [x28, #{signal_stack}]` | caught |
| 1317 | `str xzr, [x11, #{top_running}]` | caught (limit) |
| 1318 | `ldr x9, [x28, #{thread_stack}]` | caught |
| 1319 | `mov sp, x9` | caught |
| 1320 | `ldp x9, x10, [sp, #{kept_system}]` | caught |
| 1321 | `msr fpcr, x9` | caught |
| 1322 | `msr tpidr_el0, x10` | caught |
| 1323 | `ldp d8, d9, [sp, #{kept_d} + 0]` | **missed** |
| 1324 | `ldp d10, d11, [sp, #{kept_d} + 16]` | **missed** |
| 1325 | `ldp d12, d13, [sp, #{kept_d} + 32]` | **missed** |
| 1326 | `ldp d14, d15, [sp, #{kept_d} + 48]` | **missed** |
| 1327 | `ldp x19, x20, [sp, #0]` | caught |
| 1328 | `ldp x21, x22, [sp, #16]` | caught |
| 1329 | `ldp x23, x24, [sp, #32]` | caught |
| 1330 | `ldp x25, x26, [sp, #48]` | caught |
| 1331 | `ldp x27, x28, [sp, #64]` | caught |
| 1332 | `ldp x29, x30, [sp, #80]` | caught |
| 1333 | `add sp, sp, #{kept}` | caught |
| 1334 | `ret` | caught |
| 1337 | `mov x28, x0` | caught |
| 1338 | `ldr x11, [x28, #{signal_stack}]` | caught |
| 1339 | `str xzr, [x11, #{top_pending}]` | caught (limit) |
| 1340 | `mov x9, #{stop}` | caught |
| 1341 | `str x9, [x28, #{end}]` | caught (limit) |
| 1342 | `mov x9, #{not_entered}` | caught |
| 1343 | `str x9, [x28, #{end_detail}]` | caught |
| 1344 | `b 7b` | caught |
