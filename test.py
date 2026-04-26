import requests
response = requests.get(input("url:"),stream=True)
max_len = 0
for i in response.headers.keys():
    max_len = max(max_len, len(i))

for i in response.headers.items():
    print(f"{i[0]:<{max_len}} | {i[1]}")
response.close()