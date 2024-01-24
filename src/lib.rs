use std::{cmp::min, collections::HashSet, result};

use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum Error {
	#[error("Alphabet cannot contain multibyte characters")]
	AlphabetMultibyteCharacters,
	#[error("Alphabet length must be at least 3")]
	AlphabetLength,
	#[error("Alphabet must contain unique characters")]
	AlphabetUniqueCharacters,
	#[error("Reached max attempts to re-generate the ID")]
	BlocklistMaxAttempts,
}

pub type Result<T> = result::Result<T, Error>;

pub fn default_blocklist() -> HashSet<String> {
	serde_json::from_str(include_str!("blocklist.json")).unwrap()
}

#[derive(Debug)]
pub struct Options {
	pub alphabet: String,
	pub min_length: u8,
	pub blocklist: HashSet<String>,
}

impl Options {
	pub fn new(
		alphabet: Option<String>,
		min_length: Option<u8>,
		blocklist: Option<HashSet<String>>,
	) -> Self {
		let mut options = Options::default();

		if let Some(alphabet) = alphabet {
			options.alphabet = alphabet;
		}
		if let Some(min_length) = min_length {
			options.min_length = min_length;
		}
		if let Some(blocklist) = blocklist {
			options.blocklist = blocklist;
		}

		options
	}
}

impl Default for Options {
	fn default() -> Self {
		Options {
			alphabet: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string(),
			min_length: 0,
			blocklist: default_blocklist(),
		}
	}
}

#[derive(Debug)]
pub struct Sqids {
	alphabet: Vec<char>,
	min_length: u8,
	blocklist: HashSet<String>,
}

impl Default for Sqids {
	fn default() -> Self {
		Sqids::new(None).unwrap()
	}
}

impl Sqids {
	pub fn new(options: Option<Options>) -> Result<Self> {
		let options = options.unwrap_or_default();
		let alphabet: Vec<char> = options.alphabet.chars().collect();

		for c in alphabet.iter() {
			if c.len_utf8() > 1 {
				return Err(Error::AlphabetMultibyteCharacters);
			}
		}

		if alphabet.len() < 3 {
			return Err(Error::AlphabetLength);
		}

		let unique_chars: HashSet<char> = alphabet.iter().cloned().collect();
		if unique_chars.len() != alphabet.len() {
			return Err(Error::AlphabetUniqueCharacters);
		}

		let lowercase_alphabet: Vec<char> =
			alphabet.iter().map(|c| c.to_ascii_lowercase()).collect();
		let filtered_blocklist: HashSet<String> = options
			.blocklist
			.iter()
			.filter_map(|word| {
				let word = word.to_lowercase();
				if word.len() >= 3 && word.chars().all(|c| lowercase_alphabet.contains(&c)) {
					Some(word)
				} else {
					None
				}
			})
			.collect();

		Ok(Sqids {
			alphabet: Self::shuffle(&alphabet),
			min_length: options.min_length,
			blocklist: filtered_blocklist,
		})
	}

	pub fn encode(&self, numbers: &[u64]) -> Result<String> {
		if numbers.is_empty() {
			return Ok(String::new());
		}

		self.encode_numbers(numbers, 0)
	}

	pub fn decode(&self, id: &str) -> Vec<u64> {
		let mut ret = Vec::new();

		if id.is_empty() {
			return ret;
		}

		let alphabet_chars: HashSet<char> = self.alphabet.iter().cloned().collect(); //字符表，转成set
		if !id.chars().all(|c| alphabet_chars.contains(&c)) { //如果发现有不存在的字符，就直接返回空数组
			return ret; 
		}

		let prefix = id.chars().next().unwrap(); //取得首字符，确认prefix
		let offset = self.alphabet.iter().position(|&c| c == prefix).unwrap(); //方向计算对应的offset
		let mut alphabet: Vec<char> =
			self.alphabet.iter().cycle().skip(offset).take(self.alphabet.len()).copied().collect();

		alphabet = alphabet.into_iter().rev().collect(); //构建和编码时相同的字符表

		let mut id = id[1..].to_string(); //删除prefix

		while !id.is_empty() {
			let separator = alphabet[0];

			let chunks: Vec<&str> = id.split(separator).collect(); //如果存在多个numbers编码后的ID，那么就存在多个chunk
			if !chunks.is_empty() {
				if chunks[0].is_empty() {
					return ret;
				}

				let alphabet_without_separator: Vec<char> =
					alphabet.iter().copied().skip(1).collect(); //去掉第一个字符的字符表
				ret.push(self.to_number(chunks[0], &alphabet_without_separator)); //反转成数字

				if chunks.len() > 1 {
					alphabet = Self::shuffle(&alphabet); //对字符表进行洗牌
				}
			}

			id = chunks[1..].join(&separator.to_string());
     //删除第一个chunk，然后用当前separator进行粘合，因为下一轮的separator已经变了
		}

		ret
	}

	fn encode_numbers(&self, numbers: &[u64], increment: usize) -> Result<String> {
		if increment > self.alphabet.len() { //步进不能大于整个字符表
			return Err(Error::BlocklistMaxAttempts);
		}
    //将numbers的长度作为初始值
    // v = numbers[i]
    // a = a + i + self.alphabet[v % self.alphabet.len()]
		let mut offset = numbers.iter().enumerate().fold(numbers.len(), |a, (i, &v)| {
			self.alphabet[v as usize % self.alphabet.len()] as usize + i + a
		}) % self.alphabet.len();
    //计算出最终的offset
		offset = (offset + increment) % self.alphabet.len();
    //在offset这个位置将整个alphabet进行前后调换
		let mut alphabet: Vec<char> =
			self.alphabet.iter().cycle().skip(offset).take(self.alphabet.len()).copied().collect();
    //取出字符表第一个字符，作为前缀字符，放在生成的ID的最前面，用来作ID首字符
		let prefix = alphabet[0];
    //将整个字符表进行逆转
		alphabet = alphabet.into_iter().rev().collect();
    //将prefix变成字符串放入Vec
		let mut ret: Vec<String> = vec![prefix.to_string()];
    //开始遍历numbers序列
		for (i, &num) in numbers.iter().enumerate() {
			  ret.push(self.to_id(num, &alphabet[1..])); //使用除了第一个字符以外的字符表进行转换

			if i < numbers.len() - 1 {
				ret.push(alphabet[0].to_string()); //放入分割符号
				alphabet = Self::shuffle(&alphabet); //再次洗牌
			}
		}

		let mut id = ret.join(""); //将所有的id进行连接

		if self.min_length as usize > id.len() { //需要生成最小字符串大于生成的id长度
			id += &alphabet[0].to_string(); //继续添加分割符号

			while self.min_length as usize - id.len() > 0 {
				alphabet = Self::shuffle(&alphabet); //洗牌

				let slice_len = min(self.min_length as usize - id.len(), alphabet.len());
				let slice: Vec<char> = alphabet.iter().take(slice_len).cloned().collect();

				id += &slice.iter().collect::<String>(); //填充垃圾字符串
			}
		}

		if self.is_blocked_id(&id) { //如果是非法的id，那么就增加步长，重新来一次
			id = self.encode_numbers(numbers, increment + 1)?;
		}

		Ok(id)
	}

	fn to_id(&self, num: u64, alphabet: &[char]) -> String {
		let mut id = Vec::new();
		let mut result = num;
    // 13 % 4  = 1, 13 / 4 = 3
    // 3 % 4 = 3,3 / 4 = 0
		loop {
			let idx = (result % alphabet.len() as u64) as usize;
			id.insert(0, alphabet[idx]);
			result /= alphabet.len() as u64;

			if result == 0 {
				break;
			}
		}

		id.into_iter().collect()
	}

	fn to_number(&self, id: &str, alphabet: &[char]) -> u64 {
		let mut result = 0;
    // idx = 3,result = 3
    // idx = 1, result = 13
		for c in id.chars() {
			let idx = alphabet.iter().position(|&x| x == c).unwrap();
			result = result * alphabet.len() as u64 + idx as u64;
		}

		result
	}

	fn shuffle(alphabet: &[char]) -> Vec<char> {
		let mut chars: Vec<char> = alphabet.to_vec(); //转化为vec

		for i in 0..(chars.len() - 1) { // 0 到 n-1
			let j = chars.len() - 1 - i; // 反向取vec中对应位置 i = 1 j = n-2
			let r = (i as u32 * j as u32 + chars[i] as u32 + chars[j] as u32) % chars.len() as u32; //计算出一个新的位置
			chars.swap(i, r as usize); //将chars[i]换成chars[r]的位置进行互换
		}

		chars
	}

	fn is_blocked_id(&self, id: &str) -> bool {
		let id = id.to_lowercase();

		for word in &self.blocklist {
			if word.len() <= id.len() {
				if id.len() <= 3 || word.len() <= 3 {
					if id == *word {
						return true;
					}
				} else if word.chars().any(|c| c.is_ascii_digit()) {
					if id.starts_with(word) || id.ends_with(word) {
						return true;
					}
				} else if id.contains(word) {
					return true;
				}
			}
		}

		false
	}
}
